import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import type { AccountEntity } from "#/account/account.entity";
import type { AppBindings } from "#/env";
import { PaymentEntity } from "./payment.entity";

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

    // Compute credits
    const creditedAmount = this.computeCredits(
      event.amount,
      bindings.config.creditsPerToken,
    );
    if (creditedAmount <= 0) {
      bindings.log.warn(
        `Payment ${event.txHash} resulted in 0 credits, skipping`,
      );
      return null;
    }

    // Save payment and credit account in a transaction
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
        creditedAmount,
        createdAt: new Date(),
      });

      const accountRepo = queryRunner.manager.getRepository(
        (await import("#/account/account.entity")).AccountEntity,
      );
      await accountRepo
        .createQueryBuilder()
        .update()
        .set({ credits: () => `credits + ${creditedAmount}` })
        .where("id = :id", { id: account.id })
        .execute();

      await queryRunner.commitTransaction();

      bindings.log.info(
        `Credited ${creditedAmount} credits to account ${account.id} from tx ${event.txHash}`,
      );
      return payment;
    } catch (e) {
      await queryRunner.rollbackTransaction();
      throw e;
    } finally {
      await queryRunner.release();
    }
  }

  computeCredits(amountInWei: bigint, creditsPerToken: number): number {
    const tokens = amountInWei / BigInt(10 ** 6);
    return Number(tokens) * creditsPerToken;
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
