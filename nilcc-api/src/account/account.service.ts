import * as crypto from "node:crypto";
import { In, type QueryRunner, type Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import {
  EntityAlreadyExists,
  EntityNotFound,
  isUniqueConstraint,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import { WorkloadEntity } from "#/workload/workload.entity";
import type { AddCreditsRequest, CreateAccountRequest } from "./account.dto";
import { AccountEntity } from "./account.entity";

const API_TOKEN_BYTE_LENGTH: number = 16;

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
        apiToken: crypto.randomBytes(API_TOKEN_BYTE_LENGTH).toString("hex"),
        createdAt: new Date(),
        credits: request.credits,
      });
    } catch (e: unknown) {
      if (isUniqueConstraint(e)) {
        throw new EntityAlreadyExists("account");
      }
      throw e;
    }
  }

  async read(bindings: AppBindings, id: string): Promise<AccountEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id });
  }

  async findByApiToken(
    bindings: AppBindings,
    apiToken: string,
  ): Promise<AccountEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ apiToken });
  }

  async list(bindings: AppBindings): Promise<AccountEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  async addCredits(
    bindings: AppBindings,
    request: AddCreditsRequest,
  ): Promise<AccountEntity> {
    const repository = this.getRepository(bindings);
    const account = await repository.findOneBy({ id: request.accountId });
    if (account === null) {
      throw new EntityNotFound("account");
    }
    account.credits += request.credits;
    await repository.save(account);
    return account;
  }

  async deductCredits(
    bindings: AppBindings,
    workloads: WorkloadEntity[],
    tx: QueryRunner,
  ): Promise<WorkloadEntity[]> {
    const accountCredits: Record<string, number> = {};
    for (const workload of workloads) {
      if (workload.status === "stopped") {
        bindings.log.debug(
          `Ignoring workload ${workload.id} because it's stopped`,
        );
        continue;
      }
      const existingCredits = accountCredits[workload.account.id];
      if (existingCredits === undefined) {
        accountCredits[workload.account.id] = workload.creditRate;
      } else {
        accountCredits[workload.account.id] =
          existingCredits + workload.creditRate;
      }
    }
    const repository = this.getRepository(bindings, tx);
    const accounts = await repository.findBy({
      id: In(Object.keys(accountCredits)),
    });
    const offenders = [];
    for (const account of accounts) {
      const delta = accountCredits[account.id];
      if (delta === undefined) {
        bindings.log.error(`Account ${account.id} was not in map`);
        continue;
      }
      bindings.log.info(
        `Deducting ${delta} credits from account ${account.id}`,
      );
      account.credits = Math.max(0, account.credits - delta);
      if (account.credits === 0) {
        const accountWorkloads = workloads.filter(
          (w) => w.account.id === account.id && w.status !== "stopped",
        );
        if (accountWorkloads.length > 0) {
          bindings.log.info(
            `Need to shutdown ${accountWorkloads.length} workloads for account ${account.id} because it no longer has credits`,
          );
          offenders.push(...accountWorkloads);
        }
      }
    }
    await repository.save(accounts);
    return offenders;
  }

  async getAccountSpending(
    bindings: AppBindings,
    accountId: string,
  ): Promise<number> {
    const repository = bindings.dataSource.getRepository(WorkloadEntity);
    const row = await repository
      .createQueryBuilder("workload")
      .where("workload.account_id = :accountId", { accountId })
      .where("workload.status != 'stopped'")
      .select("SUM(workload.creditRate) as sum")
      .getRawOne();
    return Number(row.sum || 0);
  }
}
