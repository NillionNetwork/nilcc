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
import type { RegisterMetalInstanceRequest } from "./metal-instance.dto";
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

  @mapError((e) => new FindEntityError(MetalInstanceEntity, e))
  async list(bindings: AppBindings): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError(MetalInstanceEntity, e))
  async read(
    bindings: AppBindings,
    metalInstanceId: string,
    tx?: QueryRunner,
  ): Promise<MetalInstanceEntity | null> {
    const repository = this.getRepository(bindings, tx);
    return await repository.findOneBy({ id: metalInstanceId });
  }

  @mapError((e) => new RemoveEntityError(MetalInstanceEntity, e))
  async remove(
    bindings: AppBindings,
    metalInstanceId: string,
  ): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const result = await repository.delete({ id: metalInstanceId });
    return result.affected ? result.affected > 0 : false;
  }

  @mapError((e) => new CreateOrUpdateEntityError(MetalInstanceEntity, e))
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
      cpus: number;
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
        "metalInstance.totalCpus - metalInstance.osReservedCpus - COALESCE(SUM(workload.cpus), 0) > :requiredCpus",
        { requiredCpus: param.cpus },
      )
      .andHaving(
        "metalInstance.totalMemory - metalInstance.osReservedMemory - COALESCE(SUM(workload.memory), 0) > :requiredMemory",
        { requiredMemory: param.memory },
      )
      .andHaving(
        "metalInstance.totalDisk - metalInstance.osReservedDisk - COALESCE(SUM(workload.disk), 0) > :requiredDisk",
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

  @mapError((e) => new UpdateEntityError(MetalInstanceEntity, e))
  async update(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    currentMetalInstance: MetalInstanceEntity,
    tx?: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    currentMetalInstance.agentVersion = metalInstance.agentVersion;
    currentMetalInstance.hostname = metalInstance.hostname;

    currentMetalInstance.totalCpus = metalInstance.cpus.total;
    currentMetalInstance.osReservedCpus = metalInstance.cpus.reserved;

    currentMetalInstance.totalMemory = metalInstance.memoryMb.total;
    currentMetalInstance.osReservedMemory = metalInstance.memoryMb.reserved;

    currentMetalInstance.totalDisk = metalInstance.diskSpaceGb.total;
    currentMetalInstance.osReservedDisk = metalInstance.diskSpaceGb.reserved;

    currentMetalInstance.gpus = metalInstance.gpus;
    currentMetalInstance.gpuModel = metalInstance.gpuModel;
    currentMetalInstance.updatedAt = new Date();
    await repository.save(currentMetalInstance);
  }

  @mapError((e) => new CreateEntityError(MetalInstanceEntity, e))
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
      totalCpus: metalInstance.cpus.total,
      osReservedCpus: metalInstance.cpus.reserved,
      totalMemory: metalInstance.memoryMb.total,
      osReservedMemory: metalInstance.memoryMb.reserved,
      totalDisk: metalInstance.diskSpaceGb.total,
      osReservedDisk: metalInstance.diskSpaceGb.reserved,
      gpus: metalInstance.gpus,
      gpuModel: metalInstance.gpuModel,
      createdAt: now,
      updatedAt: now,
    });
    await repository.save(newMetalInstance);
  }
}
