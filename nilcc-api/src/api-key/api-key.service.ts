import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import { EntityNotFound } from "#/common/errors";
import type { AppBindings } from "#/env";
import type { CreateApiKeyRequest, UpdateApiKeyRequest } from "./api-key.dto";
import { ApiKeyEntity } from "./api-key.entity";

export class ApiKeyService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<ApiKeyEntity> {
    if (tx) {
      return tx.manager.getRepository(ApiKeyEntity);
    }
    return bindings.dataSource.getRepository(ApiKeyEntity);
  }

  async create(
    bindings: AppBindings,
    request: CreateApiKeyRequest,
    tx?: QueryRunner,
  ): Promise<ApiKeyEntity> {
    const repository = this.getRepository(bindings, tx);
    const now = new Date();
    return await repository.save({
      id: uuidv4(),
      accountId: request.accountId,
      type: request.type,
      active: request.active,
      createdAt: now,
      updatedAt: now,
    });
  }

  async listByAccount(
    bindings: AppBindings,
    accountId: string,
    tx?: QueryRunner,
  ): Promise<ApiKeyEntity[]> {
    const repository = this.getRepository(bindings, tx);
    return await repository.find({
      where: { accountId },
      order: { createdAt: "DESC" },
    });
  }

  async read(
    bindings: AppBindings,
    id: string,
    tx?: QueryRunner,
  ): Promise<ApiKeyEntity | null> {
    const repository = this.getRepository(bindings, tx);
    return await repository.findOne({ where: { id } });
  }

  async update(
    bindings: AppBindings,
    request: UpdateApiKeyRequest,
    tx?: QueryRunner,
  ): Promise<ApiKeyEntity> {
    const repository = this.getRepository(bindings, tx);
    const apiKey = await repository.findOne({
      where: { id: request.id },
    });
    if (!apiKey) {
      throw new EntityNotFound("api key");
    }

    if (request.type !== undefined) {
      apiKey.type = request.type;
    }
    if (request.active !== undefined) {
      apiKey.active = request.active;
    }
    apiKey.updatedAt = new Date();

    return await repository.save(apiKey);
  }

  async delete(
    bindings: AppBindings,
    id: string,
    tx?: QueryRunner,
  ): Promise<void> {
    const repository = this.getRepository(bindings, tx);
    const result = await repository.delete({ id });
    if ((result.affected ?? 0) === 0) {
      throw new EntityNotFound("api key");
    }
  }

  async findActiveById(
    bindings: AppBindings,
    id: string,
  ): Promise<ApiKeyEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOne({
      where: { id, active: true },
    });
  }
}
