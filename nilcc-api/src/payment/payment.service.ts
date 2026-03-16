import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import type { AccountEntity } from "#/account/account.entity";
import { uint256ToNil } from "#/common/nil";
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

    if (event.amount === 0n) {
      bindings.log.warn(`Payment ${event.txHash} has zero amount, skipping`);
      return null;
    }

    // Convert uint256 to decimal NIL at the boundary
    const depositedAmount = uint256ToNil(event.amount);

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
        depositedAmount,
        createdAt: new Date(),
      });

      const accountRepo = queryRunner.manager.getRepository(
        (await import("#/account/account.entity")).AccountEntity,
      );
      await accountRepo
        .createQueryBuilder()
        .update()
        .set({
          balance: () => `balance + ${depositedAmount}`,
        })
        .where("id = :id", { id: account.id })
        .execute();

      await queryRunner.commitTransaction();

      bindings.log.info(
        `Deposited ${depositedAmount} NIL to account ${account.id} from tx ${event.txHash}`,
      );
      return payment;
    } catch (e) {
      await queryRunner.rollbackTransaction();
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
