import type { QueryRunner, Repository } from "typeorm";
import { ArtifactEntity } from "#/artifact/artifact.entity";
import {
  EntityNotFound,
  MetalInstanceManagingWorkloads,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  HeartbeatRequest,
  HeartbeatResponse,
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
  ): Promise<HeartbeatResponse> {
    const repository = this.getRepository(bindings, tx);
    const instances = await repository.find({
      where: { id: request.metalInstanceId },
      relations: ["workloads", "workloads.account"],
    });
    if (instances.length === 0) {
      throw new EntityNotFound("metal instance");
    }
    const expectedArtifacts = await tx.manager
      .getRepository(ArtifactEntity)
      .find();
    const instance = instances[0];
    const now = bindings.services.time.getTime();
    bindings.metrics.metalInstanceHeartbeats.labels({ id: instance.id }).inc();
    if (now.getMinutes() !== instance.lastSeenAt.getMinutes()) {
      bindings.log.info(
        `Need to deduct credits for ${instance.workloads.length} workloads running on agent ${instance.id}`,
      );
      const offendingWorkloads = await bindings.services.account.deductCredits(
        bindings,
        instance.workloads,
        tx,
      );
      if (offendingWorkloads.length > 0) {
        bindings.log.info(
          `Have ${offendingWorkloads.length} offending workloads that need to be shutdown`,
        );
        try {
          for (const workload of offendingWorkloads) {
            bindings.services.workload.stop(
              bindings,
              workload.id,
              workload.account,
              tx,
            );
            bindings.metrics.deactivatedWorkloads.inc();
          }
        } catch (error: unknown) {
          bindings.log.error(
            `Failed to stop workloads: ${JSON.stringify(error)}`,
          );
        }
      }
    }
    instance.lastSeenAt = now;
    instance.availableArtifactVersions = request.availableArtifactVersions;

    await repository.save(instance);
    return {
      metalInstanceId: instance.id,
      expectedArtifactVersions: expectedArtifacts.map((a) => a.version),
    };
  }

  async findWithFreeResources(
    param: {
      cpus: number;
      memory: number;
      disk: number;
      gpus: number;
      artifactsVersion: string;
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
          now: bindings.services.time.getTime(),
          threshold: bindings.config.metalInstancesIdleThresholdSeconds,
        },
      )
      .leftJoin("metalInstance.workloads", "workload")
      .groupBy("metalInstance.id")
      .having(
        "metalInstance.totalCpus - metalInstance.reservedCpus - COALESCE(SUM(workload.cpus), 0) > :requiredCpus",
        { requiredCpus: param.cpus },
      )
      .andHaving(
        "metalInstance.totalMemory - metalInstance.reservedMemory - COALESCE(SUM(workload.memory), 0) > :requiredMemory",
        { requiredMemory: param.memory },
      )
      .andHaving(
        "metalInstance.totalDisk - metalInstance.reservedDisk - COALESCE(SUM(workload.disk), 0) > :requiredDisk",
        { requiredDisk: param.disk },
      )
      .andHaving(
        "metalInstance.gpus - COALESCE(SUM(workload.gpus), 0) >= :requiredGpu",
        { requiredGpu: param.gpus },
      );

    const instances = await queryBuilder.getMany();
    return instances.filter((instance) =>
      instance.availableArtifactVersions.includes(param.artifactsVersion),
    );
  }

  async update(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
    currentMetalInstance: MetalInstanceEntity,
    tx: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    const now = bindings.services.time.getTime();
    currentMetalInstance.agentVersion = metalInstance.agentVersion;
    currentMetalInstance.hostname = metalInstance.hostname;
    currentMetalInstance.token = metalInstance.token;

    currentMetalInstance.totalCpus = metalInstance.cpus.total;
    currentMetalInstance.reservedCpus = metalInstance.cpus.reserved;

    currentMetalInstance.totalMemory = metalInstance.memoryMb.total;
    currentMetalInstance.reservedMemory = metalInstance.memoryMb.reserved;

    currentMetalInstance.totalDisk = metalInstance.diskSpaceGb.total;
    currentMetalInstance.reservedDisk = metalInstance.diskSpaceGb.reserved;

    currentMetalInstance.gpus = metalInstance.gpus;
    currentMetalInstance.gpuModel = metalInstance.gpuModel;
    currentMetalInstance.updatedAt = now;
    currentMetalInstance.lastSeenAt = now;
    await repository.save(currentMetalInstance);
  }

  async create(
    bindings: AppBindings,
    request: RegisterMetalInstanceRequest,
    tx: QueryRunner,
  ) {
    const repository = this.getRepository(bindings, tx);
    const now = bindings.services.time.getTime();
    const newMetalInstance = repository.create({
      id: request.metalInstanceId,
      agentVersion: request.agentVersion,
      token: request.token,
      publicIp: request.publicIp,
      hostname: request.hostname,
      totalCpus: request.cpus.total,
      reservedCpus: request.cpus.reserved,
      totalMemory: request.memoryMb.total,
      reservedMemory: request.memoryMb.reserved,
      totalDisk: request.diskSpaceGb.total,
      reservedDisk: request.diskSpaceGb.reserved,
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
