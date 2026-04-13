import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import type { AccountEntity } from "#/account/account.entity";
import { isUniqueConstraint } from "#/common/errors";
import {
  microdollarsToUsd,
  nilToMicrodollars,
  uint256ToNil,
} from "#/common/nil";
import type { AppBindings } from "#/env";
import { PaymentEntity } from "./payment.entity";

export class NilPriceUnavailableError extends Error {
  constructor(txHash: string) {
    super(`NIL price unavailable, cannot process payment ${txHash}`);
  }
}

export class PaymentService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<PaymentEntity> {
    if (tx) {
      return tx.manager.getRepository(PaymentEntity);
    }
    return bindings.dataSource.getRepository(PaymentEntity);
  }

  async processEvent(
    bindings: AppBindings,
    event: {
      txHash: string;
      logIndex: number;
      blockNumber: number;
      fromAddress: string;
      amount: bigint;
      digest: string;
    },
  ): Promise<PaymentEntity | null> {
    const repository = this.getRepository(bindings);

    // Idempotency check
    const existing = await repository.findOneBy({
      txHash: event.txHash,
    });
    if (existing) {
      bindings.log.debug(`Payment already processed: ${event.txHash}`);
      return existing;
    }

    // Find account by wallet address
    const account = await bindings.services.account.findByWalletAddress(
      bindings,
      event.fromAddress,
    );
    if (!account) {
      bindings.log.warn(
        `No account found for wallet ${event.fromAddress}, skipping payment ${event.txHash}`,
      );
      return null;
    }

    if (event.amount === 0n) {
      bindings.log.warn(`Payment ${event.txHash} has zero amount, skipping`);
      return null;
    }

    // Convert uint256 to decimal NIL at the boundary
    const nilAmount = uint256ToNil(event.amount);

    // Fetch live NIL price and convert to integer microdollars
    const nilPrice = await bindings.services.nilPrice.fetchNilPrice();
    if (nilPrice === null) {
      throw new NilPriceUnavailableError(event.txHash);
    }
    const depositedMicrodollars = nilToMicrodollars(nilAmount, nilPrice);
    if (depositedMicrodollars <= 0) {
      bindings.log.warn(
        `Payment ${event.txHash} converts to $0 USD (${nilAmount} NIL @ $${nilPrice}), skipping`,
      );
      return null;
    }

    // Save payment and update account balance in a transaction
    const queryRunner = bindings.dataSource.createQueryRunner();
    try {
      await queryRunner.connect();
      await queryRunner.startTransaction();

      const paymentRepo = queryRunner.manager.getRepository(PaymentEntity);
      const payment = await paymentRepo.save({
        id: uuidv4(),
        txHash: event.txHash,
        logIndex: event.logIndex,
        blockNumber: event.blockNumber,
        fromAddress: event.fromAddress.toLowerCase(),
        amount: event.amount.toString(),
        digest: event.digest,
        account: { id: account.id } as AccountEntity,
        nilAmount,
        nilPriceAtDeposit: nilPrice,
        depositedAmountUsd: depositedMicrodollars,
        createdAt: new Date(),
      });

      await queryRunner.query(
        `UPDATE "accounts" SET "balance" = "balance" + $1 WHERE "id" = $2`,
        [depositedMicrodollars, account.id],
      );

      await queryRunner.commitTransaction();

      bindings.log.info(
        `Deposited $${microdollarsToUsd(depositedMicrodollars)} USD (${nilAmount} NIL @ $${nilPrice}) to account ${account.id} from tx ${event.txHash}`,
      );
      return payment;
    } catch (e) {
      await queryRunner.rollbackTransaction();
      if (isUniqueConstraint(e)) {
        bindings.log.info(`Payment already processed: ${event.txHash}`);
        return null;
      }
      throw e;
    } finally {
      await queryRunner.release();
    }
  }

  async listByAccount(
    bindings: AppBindings,
    accountId: string,
  ): Promise<PaymentEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find({
      where: { account: { id: accountId } },
      order: { createdAt: "DESC" },
    });
  }
}
