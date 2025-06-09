import type { QueryRunner, Repository } from "typeorm";
import {
  CreateEntityError,
  FindEntityError,
  GetRepositoryError,
  InstancesNotAvailable,
  mapError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import { metalInstanceService } from "#/metal-instance/metal-instance.service";
import type {
  CreateWorkloadRequest,
  UpdateWorkloadRequest,
} from "./workload.dto";
import { WorkloadEntity } from "./workload.entity";

export class WorkloadService {
  @mapError((e) => new GetRepositoryError(e))
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<WorkloadEntity> {
    if (tx) {
      return tx.manager.getRepository(WorkloadEntity);
    }
    return bindings.dataSource.getRepository(WorkloadEntity);
  }

  @mapError((e) => new CreateEntityError(WorkloadEntity, e))
  async create(
    bindings: AppBindings,
    workload: CreateWorkloadRequest,
    tx: QueryRunner,
  ): Promise<WorkloadEntity> {
    const metalInstances = await metalInstanceService.findWithFreeResources(
      {
        cpu: workload.cpu,
        memory: workload.memory,
        disk: workload.disk,
        gpu: workload.gpu,
      },
      bindings,
      tx,
    );

    if (metalInstances.length === 0) {
      throw new InstancesNotAvailable();
    }

    const metalInstance =
      metalInstances[Math.floor(Math.random() * metalInstances.length)];

    // Assign the first available metal instance to the workload
    const repository = this.getRepository(bindings, tx);
    const now = new Date();
    const entity = repository.create({
      ...workload,
      metalInstance,
      createdAt: now,
      updatedAt: now,
    });
    return await repository.save(entity);
  }

  @mapError((e) => new FindEntityError(WorkloadEntity, e))
  async list(bindings: AppBindings): Promise<WorkloadEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError(WorkloadEntity, e))
  async read(
    bindings: AppBindings,
    workloadId: string,
  ): Promise<WorkloadEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id: workloadId });
  }

  @mapError((e) => new UpdateEntityError(WorkloadEntity, e))
  async update(
    bindings: AppBindings,
    payload: UpdateWorkloadRequest,
    tx?: QueryRunner,
  ): Promise<boolean> {
    const repository = this.getRepository(bindings, tx);
    const updated = await repository.update(
      { id: payload.id },
      {
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        dockerCompose: payload.dockerCompose,
        envVars: payload.envVars,
        serviceToExpose: payload.serviceToExpose,
        servicePortToExpose: payload.servicePortToExpose,
        memory: payload.memory,
        cpu: payload.cpu,
        updatedAt: new Date(),
      },
    );
    return updated.affected ? updated.affected > 0 : false;
  }

  @mapError((e) => new RemoveEntityError(WorkloadEntity, e))
  async remove(bindings: AppBindings, workloadId: string): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const result = await repository.delete({ id: workloadId });
    return result.affected ? result.affected > 0 : false;
  }
}

export const workloadService = new WorkloadService();
