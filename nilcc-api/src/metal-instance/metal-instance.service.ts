import type { QueryRunner, Repository } from "typeorm";
import {
  EntityNotFound,
  MetalInstanceManagingWorkloads,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  HeartbeatRequest,
  RegisterMetalInstanceRequest,
} from "./metal-instance.dto";
import { MetalInstanceEntity } from "./metal-instance.entity";

export class MetalInstanceService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<MetalInstanceEntity> {
    if (tx) {
      return tx.manager.getRepository(MetalInstanceEntity);
    }
    return bindings.dataSource.getRepository(MetalInstanceEntity);
  }

  async list(bindings: AppBindings): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  async read(
    bindings: AppBindings,
    metalInstanceId: string,
    tx?: QueryRunner,
  ): Promise<MetalInstanceEntity | null> {
    const repository = this.getRepository(bindings, tx);
    return await repository.findOneBy({ id: metalInstanceId });
  }

  async remove(bindings: AppBindings, metalInstanceId: string): Promise<void> {
    const repository = this.getRepository(bindings);
    const instances = await repository.find({
      where: { id: metalInstanceId },
      relations: ["workloads"],
    });
    if (instances.length === 0) {
      throw new EntityNotFound("workload");
    }
    const instance = instances[0];
    if (instance.workloads.length > 0) {
      throw new MetalInstanceManagingWorkloads();
    }
    await repository.delete({ id: metalInstanceId });
    await bindings.services.dns.metalInstances.deleteRecord(
      metalInstanceId,
      "CNAME",
    );
  }

  async createOrUpdate(
    bindings: AppBindings,
    request: RegisterMetalInstanceRequest,
    tx: QueryRunner,
  ) {
    const maybeMetalInstance = await this.read(
      bindings,
      request.metalInstanceId,
      tx,
    );
    if (maybeMetalInstance) {
      return this.update(bindings, request, maybeMetalInstance, tx);
    }
    return this.create(bindings, request, tx);
  }

  async heartbeat(
    bindings: AppBindings,
    request: HeartbeatRequest,
    tx: QueryRunner,
  ) {
    const metalInstance = await this.read(
      bindings,
      request.metalInstanceId,
      tx,
    );
    if (metalInstance === null) {
      throw new EntityNotFound("metal instance");
    }
    const repository = this.getRepository(bindings, tx);
    metalInstance.lastSeenAt = new Date();
    await repository.save(metalInstance);
  }

  async findWithFreeResources(
    param: {
      cpus: number;
      memory: number;
      disk: number;
      gpus: number;
    },
    bindings: AppBindings,
    tx: QueryRunner,
  ): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings, tx);
    const queryBuilder = repository
      .createQueryBuilder("metalInstance")
      .where(
        "EXTRACT(EPOCH FROM (:now - metalInstance.lastSeenAt)) < :threshold",
        {
          now: new Date(),
          threshold: bindings.config.metalInstancesIdleThresholdSeconds,
        },
      )
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
      )
      .andHaving(
        "metalInstance.gpus - COALESCE(SUM(workload.gpus), 0) >= :requiredGpu",
        { requiredGpu: param.gpus },
      );

    return await queryBuilder.getMany();
  }

  async update(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    currentMetalInstance: MetalInstanceEntity,
    tx: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    currentMetalInstance.agentVersion = metalInstance.agentVersion;
    currentMetalInstance.hostname = metalInstance.hostname;
    currentMetalInstance.token = metalInstance.token;

    currentMetalInstance.totalCpus = metalInstance.cpus.total;
    currentMetalInstance.osReservedCpus = metalInstance.cpus.reserved;

    currentMetalInstance.totalMemory = metalInstance.memoryMb.total;
    currentMetalInstance.osReservedMemory = metalInstance.memoryMb.reserved;

    currentMetalInstance.totalDisk = metalInstance.diskSpaceGb.total;
    currentMetalInstance.osReservedDisk = metalInstance.diskSpaceGb.reserved;

    currentMetalInstance.gpus = metalInstance.gpus;
    currentMetalInstance.gpuModel = metalInstance.gpuModel;
    currentMetalInstance.updatedAt = new Date();
    currentMetalInstance.lastSeenAt = new Date();
    await repository.save(currentMetalInstance);
  }

  async create(
    bindings: AppBindings,
    request: RegisterMetalInstanceRequest,
    tx: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    const now = new Date();
    const newMetalInstance = repository.create({
      id: request.metalInstanceId,
      agentVersion: request.agentVersion,
      token: request.token,
      publicIp: request.publicIp,
      hostname: request.hostname,
      totalCpus: request.cpus.total,
      osReservedCpus: request.cpus.reserved,
      totalMemory: request.memoryMb.total,
      osReservedMemory: request.memoryMb.reserved,
      totalDisk: request.diskSpaceGb.total,
      osReservedDisk: request.diskSpaceGb.reserved,
      gpus: request.gpus,
      gpuModel: request.gpuModel,
      createdAt: now,
      updatedAt: now,
      lastSeenAt: now,
    });
    bindings.services.dns.metalInstances.createRecord(
      request.metalInstanceId,
      request.publicIp,
      "A",
    );

    await repository.save(newMetalInstance);
  }
}
