import type { QueryRunner, Repository } from "typeorm";
import {
  CreateEntityError,
  CreateOrUpdateEntityError,
  FindEntityError,
  GetRepositoryError,
  mapError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  RegisterMetalInstanceRequest,
  SyncMetalInstanceRequest,
} from "./metal-instance.dto";
import { MetalInstanceEntity } from "./metal-instance.entity";

export class MetalInstanceService {
  @mapError((e) => new GetRepositoryError(e))
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<MetalInstanceEntity> {
    if (tx) {
      return tx.manager.getRepository(MetalInstanceEntity);
    }
    return bindings.dataSource.getRepository(MetalInstanceEntity);
  }

  @mapError((e) => new FindEntityError(e))
  async list(bindings: AppBindings): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError(e))
  async read(
    bindings: AppBindings,
    metalInstanceId: string,
    tx?: QueryRunner,
  ): Promise<MetalInstanceEntity | null> {
    const repository = this.getRepository(bindings, tx);
    return await repository.findOneBy({ id: metalInstanceId });
  }

  @mapError((e) => new RemoveEntityError(e))
  async remove(
    bindings: AppBindings,
    metalInstanceId: string,
  ): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const result = await repository.delete({ id: metalInstanceId });
    return result.affected ? result.affected > 0 : false;
  }

  @mapError((e) => new CreateOrUpdateEntityError(e))
  async createOrUpdate(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    tx?: QueryRunner,
  ) {
    const maybeMetalInstance = await this.read(bindings, metalInstance.id, tx);
    if (maybeMetalInstance) {
      return this.update(bindings, metalInstance, maybeMetalInstance, tx);
    }
    return this.create(bindings, metalInstance, tx);
  }

  async findWithFreeResources(
    param: {
      cpu: number;
      memory: number;
      disk: number;
      gpu: number | undefined;
    },
    bindings: AppBindings,
    tx: QueryRunner,
  ): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings, tx);
    let queryBuilder = repository
      .createQueryBuilder("metalInstance")
      .leftJoin("metalInstance.workloads", "workload")
      .groupBy("metalInstance.id")
      .having(
        "metalInstance.cpu - COALESCE(SUM(workload.cpu), 0) > :requiredCpus",
        { requiredCpus: param.cpu },
      )
      .andHaving(
        "metalInstance.memory - COALESCE(SUM(workload.memory), 0) > :requiredMemory",
        { requiredMemory: param.memory },
      )
      .andHaving(
        "metalInstance.disk - COALESCE(SUM(workload.disk), 0) > :requiredDisk",
        { requiredDisk: param.disk },
      );

    if (param.gpu) {
      queryBuilder = queryBuilder.andHaving(
        "metalInstance.gpu - COALESCE(SUM(workload.gpu), 0) >= :requiredGpu",
        { requiredGpu: param.gpu },
      );
    }

    const result = await queryBuilder.getMany();

    return result;
  }

  @mapError((e) => new UpdateEntityError({ cause: e }))
  async update(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    currentMetalInstance: MetalInstanceEntity,
    tx?: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    currentMetalInstance.agentVersion = metalInstance.agentVersion;
    currentMetalInstance.hostname = metalInstance.hostname;
    currentMetalInstance.cpu = metalInstance.cpu;
    currentMetalInstance.memory = metalInstance.memory;
    currentMetalInstance.disk = metalInstance.disk;
    currentMetalInstance.gpu = metalInstance.gpu;
    currentMetalInstance.gpuModel = metalInstance.gpuModel;
    currentMetalInstance.ipAddress = metalInstance.ipAddress;
    currentMetalInstance.updatedAt = new Date();
    await repository.save(currentMetalInstance);
  }

  @mapError((e) => new CreateEntityError({ cause: e }))
  async create(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    tx: QueryRunner | undefined,
  ) {
    const repository = this.getRepository(bindings, tx);
    const now = new Date();
    const newMetalInstance = repository.create({
      id: metalInstance.id,
      agentVersion: metalInstance.agentVersion,
      hostname: metalInstance.hostname,
      cpu: metalInstance.cpu,
      memory: metalInstance.memory,
      disk: metalInstance.disk,
      gpu: metalInstance.gpu,
      gpuModel: metalInstance.gpuModel,
      ipAddress: metalInstance.ipAddress,
      createdAt: now,
      updatedAt: now,
    });
    await repository.save(newMetalInstance);
  }

  async sync(
    bindings: AppBindings,
    payload: SyncMetalInstanceRequest,
    tx: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    return await repository.findOne({
      where: { id: payload.id },
      relations: ["workloads"],
    });
  }
}

export const metalInstanceService = new MetalInstanceService();
