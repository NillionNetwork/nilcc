import * as crypto from "node:crypto";
import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import { EntityAlreadyExists, isUniqueConstraint } from "#/common/errors";
import type { AppBindings } from "#/env";
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

  async create(bindings: AppBindings, name: string): Promise<AccountEntity> {
    const repository = this.getRepository(bindings);
    try {
      return await repository.save({
        id: uuidv4(),
        name,
        apiToken: crypto.randomBytes(API_TOKEN_BYTE_LENGTH).toString("hex"),
        createdAt: new Date(),
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
}
