import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import {
  EntityAlreadyExists,
  EntityNotFound,
  isUniqueConstraint,
} from "#/common/errors";
import {
  isBalanceDepleted,
  microdollarsToUsd,
  usdToMicrodollars,
} from "#/common/nil";
import type { AppBindings } from "#/env";
import { WorkloadEntity } from "#/workload/workload.entity";
import type {
  AddBalanceRequest,
  CreateAccountRequest,
  UpdateAccountRequest,
} from "./account.dto";
import { AccountEntity } from "./account.entity";

export class AccountService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<AccountEntity> {
    if (tx) {
      return tx.manager.getRepository(AccountEntity);
    }
    return bindings.dataSource.getRepository(AccountEntity);
  }

  async create(
    bindings: AppBindings,
    request: CreateAccountRequest,
  ): Promise<AccountEntity> {
    const repository = this.getRepository(bindings);
    try {
      return await repository.save({
        id: uuidv4(),
        name: request.name,
        walletAddress: request.walletAddress.toLowerCase(),
        createdAt: new Date(),
        balance: usdToMicrodollars(request.balance),
      });
    } catch (e: unknown) {
      if (isUniqueConstraint(e)) {
        throw new EntityAlreadyExists("account");
      }
      throw e;
    }
  }

  async update(
    bindings: AppBindings,
    request: UpdateAccountRequest,
    tx: QueryRunner,
  ): Promise<AccountEntity> {
    const repository = this.getRepository(bindings, tx);
    const account = await repository.findOneBy({ id: request.accountId });
    if (account === null) {
      throw new EntityNotFound("account");
    }
    account.name = request.name;
    return await repository.save(account);
  }

  async read(bindings: AppBindings, id: string): Promise<AccountEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id });
  }

  async findByWalletAddress(
    bindings: AppBindings,
    walletAddress: string,
  ): Promise<AccountEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({
      walletAddress: walletAddress.toLowerCase(),
    });
  }

  async findOrCreateByWallet(
    bindings: AppBindings,
    walletAddress: string,
  ): Promise<AccountEntity> {
    const normalized = walletAddress.toLowerCase();
    const existing = await this.findByWalletAddress(bindings, normalized);
    if (existing) {
      return existing;
    }
    const repository = this.getRepository(bindings);
    return await repository.save({
      id: uuidv4(),
      name: normalized.slice(0, 32),
      walletAddress: normalized,
      createdAt: new Date(),
      balance: 0,
    });
  }

  async list(bindings: AppBindings): Promise<AccountEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  async addBalance(
    bindings: AppBindings,
    request: AddBalanceRequest,
  ): Promise<AccountEntity> {
    const repository = this.getRepository(bindings);
    const account = await repository.findOneBy({ id: request.accountId });
    if (account === null) {
      throw new EntityNotFound("account");
    }
    const delta = usdToMicrodollars(request.balance);
    await repository.query(
      `UPDATE "accounts" SET "balance" = "balance" + $1 WHERE "id" = $2`,
      [delta, account.id],
    );
    return await repository.findOneByOrFail({ id: account.id });
  }

  async deductBalance(
    bindings: AppBindings,
    workloads: WorkloadEntity[],
    tx: QueryRunner,
  ): Promise<WorkloadEntity[]> {
    const accountCosts: Record<string, number> = {};
    for (const workload of workloads) {
      if (workload.status === "stopped") {
        bindings.log.debug(
          `Ignoring workload ${workload.id} because it's stopped`,
        );
        continue;
      }
      const cost = workload.usdCostPerMin;
      const existing = accountCosts[workload.account.id];
      if (existing === undefined) {
        accountCosts[workload.account.id] = cost;
      } else {
        accountCosts[workload.account.id] = existing + cost;
      }
    }
    const repository = this.getRepository(bindings, tx);
    const accountIds = Object.keys(accountCosts);
    if (accountIds.length === 0) {
      return [];
    }
    const accounts = await repository
      .createQueryBuilder("account")
      .setLock("pessimistic_write")
      .where("account.id IN (:...ids)", { ids: accountIds })
      .getMany();
    const offenders = [];
    for (const account of accounts) {
      const delta = accountCosts[account.id];
      if (delta === undefined) {
        bindings.log.error(`Account ${account.id} was not in map`);
        continue;
      }
      bindings.log.info(
        `Deducting $${microdollarsToUsd(delta)} USD from account ${account.id}`,
      );
      account.balance = Math.max(0, account.balance - delta);
      if (isBalanceDepleted(account.balance)) {
        account.balance = 0;
        const accountWorkloads = workloads.filter(
          (w) => w.account.id === account.id && w.status !== "stopped",
        );
        if (accountWorkloads.length > 0) {
          bindings.log.info(
            `Need to shutdown ${accountWorkloads.length} workloads for account ${account.id} because it no longer has balance`,
          );
          offenders.push(...accountWorkloads);
        }
      }
    }
    await repository.save(accounts);
    return offenders;
  }

  async getAccountUsdSpending(
    bindings: AppBindings,
    accountId: string,
  ): Promise<number> {
    const repository = bindings.dataSource.getRepository(WorkloadEntity);
    const row = await repository
      .createQueryBuilder("workload")
      .where("workload.account_id = :accountId", { accountId })
      .andWhere("workload.status != 'stopped'")
      .select("SUM(workload.usd_cost_per_min) as sum")
      .getRawOne();
    return Number(row.sum || 0);
  }
}
