import type { Logger } from "pino";
import { createPublicClient, http } from "viem";
import type { AppBindings } from "#/env";
import { BlockCursorEntity } from "./block-cursor.entity";
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

  start(): void {
    const { rpcUrl, burnContractAddress } = this.bindings.config;
    if (!rpcUrl || !burnContractAddress) {
      this.log.info(
        "Payment poller disabled: missing rpcUrl or burnContractAddress config",
      );
      return;
    }

    const intervalMs = this.bindings.config.paymentPollerIntervalMs;
    this.log.info(`Starting payment poller with interval ${intervalMs}ms`);

    // Run immediately, then on interval
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

    // Get block cursor
    const cursorRepo =
      this.bindings.dataSource.getRepository(BlockCursorEntity);
    const cursor = await cursorRepo.findOneBy({ id: CURSOR_ID });
    const fromBlock = cursor
      ? BigInt(cursor.lastProcessedBlock) + 1n
      : BigInt(this.bindings.config.paymentStartBlock);

    // Get current block
    const currentBlock = await client.getBlockNumber();
    if (fromBlock > currentBlock) {
      this.log.debug(
        `No new blocks to process (cursor: ${fromBlock}, current: ${currentBlock})`,
      );
      return;
    }

    // Clamp range
    const toBlock =
      currentBlock - fromBlock > BigInt(paymentPollerMaxBlockRange)
        ? fromBlock + BigInt(paymentPollerMaxBlockRange) - 1n
        : currentBlock;

    this.log.info(
      `Polling blocks ${fromBlock} to ${toBlock} for LogBurnWithDigest events`,
    );

    // Fetch logs
    const logs = await client.getLogs({
      address: burnContractAddress as `0x${string}`,
      event: burnWithDigestEventAbi[0],
      fromBlock,
      toBlock,
    });

    this.log.info(`Found ${logs.length} LogBurnWithDigest events`);

    // Process each log
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

    // Update cursor. If any event failed, only advance up to the block before
    // the first failure so failed events are retried in the next poll.
    let nextCursorBlock: bigint | null = toBlock;
    if (firstFailedBlock !== null) {
      if (firstFailedBlock <= fromBlock) {
        // Would only happen if firstFailedBlock == fromBlock, other states should be impossible
        nextCursorBlock = null;
      } else {
        nextCursorBlock = firstFailedBlock - 1n;
      }
    }

    if (nextCursorBlock === null) {
      this.log.warn(
        `Not advancing block cursor due to processing failure at block ${firstFailedBlock?.toString()}`,
      );
      return;
    }

    if (cursor) {
      cursor.lastProcessedBlock = nextCursorBlock.toString();
      cursor.updatedAt = new Date();
      await cursorRepo.save(cursor);
    } else {
      await cursorRepo.save({
        id: CURSOR_ID,
        lastProcessedBlock: nextCursorBlock.toString(),
        updatedAt: new Date(),
      });
    }

    this.log.debug(`Updated block cursor to ${nextCursorBlock}`);
  }
}
