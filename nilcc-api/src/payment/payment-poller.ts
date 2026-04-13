import type { Logger } from "pino";
import { createPublicClient, http } from "viem";
import type { AppBindings } from "#/env";
import { burnWithDigestEventAbi } from "./burn-contract";
import type { PaymentService } from "./payment.service";

const CURSOR_ID = "payment-poller";

export class PaymentPoller {
  private intervalId: NodeJS.Timeout | null = null;
  private polling = false;
  private log: Logger;

  constructor(
    private bindings: AppBindings,
    private paymentService: PaymentService,
  ) {
    this.log = bindings.log.child({ component: "payment-poller" });
  }

  async start(): Promise<void> {
    const { rpcUrl, burnContractAddress } = this.bindings.config;
    if (!rpcUrl || !burnContractAddress) {
      this.log.info(
        "Payment poller disabled: missing rpcUrl or burnContractAddress config",
      );
      return;
    }

    // Seed the cursor row so SELECT ... FOR UPDATE always finds a row to lock.
    const seedBlock = (
      BigInt(this.bindings.config.paymentStartBlock) - 1n
    ).toString();
    await this.bindings.dataSource.query(
      `INSERT INTO block_cursors (id, last_processed_block, updated_at)
       VALUES ($1, $2, NOW())
       ON CONFLICT (id) DO NOTHING`,
      [CURSOR_ID, seedBlock],
    );

    const intervalMs = this.bindings.config.paymentPollerIntervalMs;
    this.log.info(`Starting payment poller with interval ${intervalMs}ms`);

    this.poll();
    this.intervalId = setInterval(() => this.poll(), intervalMs);
  }

  stop(): void {
    if (this.intervalId) {
      clearInterval(this.intervalId);
      this.intervalId = null;
      this.log.info("Payment poller stopped");
    }
  }

  private async poll(): Promise<void> {
    if (this.polling) {
      this.log.debug("Skipping poll: previous poll still running");
      return;
    }

    this.polling = true;
    try {
      await this.doPoll();
    } catch (e) {
      this.log.error(`Payment poller error: ${e}`);
    } finally {
      this.polling = false;
    }
  }

  private async doPoll(): Promise<void> {
    const { rpcUrl, burnContractAddress, paymentPollerMaxBlockRange } =
      this.bindings.config;
    if (!rpcUrl || !burnContractAddress) {
      this.log.info(
        "Payment poller disabled: missing rpcUrl or burnContractAddress config",
      );
      return;
    }

    const client = createPublicClient({
      transport: http(rpcUrl),
    });

    const queryRunner = this.bindings.dataSource.createQueryRunner();
    await queryRunner.connect();
    await queryRunner.startTransaction();
    try {
      // Single-writer gate across replicas: blocks until any concurrent poll commits,
      // then reads the already-advanced cursor and processes only the new range.
      const rows: Array<{ last_processed_block: string }> =
        await queryRunner.query(
          `SELECT last_processed_block FROM block_cursors
           WHERE id = $1 FOR UPDATE`,
          [CURSOR_ID],
        );
      const fromBlock = BigInt(rows[0].last_processed_block) + 1n;

      const currentBlock = await client.getBlockNumber();
      if (fromBlock > currentBlock) {
        this.log.debug(
          `No new blocks to process (cursor: ${fromBlock}, current: ${currentBlock})`,
        );
        await queryRunner.commitTransaction();
        return;
      }

      const toBlock =
        currentBlock - fromBlock > BigInt(paymentPollerMaxBlockRange)
          ? fromBlock + BigInt(paymentPollerMaxBlockRange) - 1n
          : currentBlock;

      this.log.info(
        `Polling blocks ${fromBlock} to ${toBlock} for LogBurnWithDigest events`,
      );

      const logs = await client.getLogs({
        address: burnContractAddress as `0x${string}`,
        event: burnWithDigestEventAbi[0],
        fromBlock,
        toBlock,
      });

      this.log.info(`Found ${logs.length} LogBurnWithDigest events`);

      let firstFailedBlock: bigint | null = null;
      for (const log of logs) {
        if (!log.transactionHash || !log.args.account || !log.args.amount) {
          this.log.warn("Skipping malformed log entry");
          continue;
        }

        try {
          await this.paymentService.processEvent(this.bindings, {
            txHash: log.transactionHash,
            logIndex: log.logIndex ?? 0,
            blockNumber: Number(log.blockNumber),
            fromAddress: log.args.account,
            amount: log.args.amount,
            digest: log.args.digest ?? "0x",
          });
        } catch (e) {
          if (firstFailedBlock === null) {
            firstFailedBlock = log.blockNumber ?? fromBlock;
          }
          this.log.warn(
            `Failed to process event from tx ${log.transactionHash}: ${e}`,
          );
        }
      }

      // If any event failed, only advance up to the block before the first failure
      // so failed events are retried next tick.
      let nextCursorBlock: bigint | null = toBlock;
      if (firstFailedBlock !== null) {
        nextCursorBlock =
          firstFailedBlock <= fromBlock ? null : firstFailedBlock - 1n;
      }

      if (nextCursorBlock === null) {
        this.log.warn(
          `Not advancing block cursor due to processing failure at block ${firstFailedBlock?.toString()}`,
        );
        await queryRunner.commitTransaction();
        return;
      }

      await queryRunner.query(
        `UPDATE block_cursors
         SET last_processed_block = $1, updated_at = NOW()
         WHERE id = $2`,
        [nextCursorBlock.toString(), CURSOR_ID],
      );
      await queryRunner.commitTransaction();

      this.log.debug(`Updated block cursor to ${nextCursorBlock}`);
    } catch (e) {
      await queryRunner.rollbackTransaction();
      throw e;
    } finally {
      await queryRunner.release();
    }
  }
}
