import type { Repository } from "typeorm";
import {
  CreateEntityError,
  FindEntityError,
  GetRepositoryError,
  mapError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type {
  CreateWorkloadRequest,
  UpdateWorkloadRequest,
} from "./workload.dto";
import { WorkloadEntity } from "./workload.entity";

export class WorkloadService {
  @mapError((e) => new GetRepositoryError({ cause: e }))
  getRepository(bindings: AppBindings): Repository<WorkloadEntity> {
    return bindings.dataSource.getRepository(WorkloadEntity);
  }

  @mapError((e) => new CreateEntityError({ cause: e }))
  async create(
    bindings: AppBindings,
    workload: CreateWorkloadRequest,
  ): Promise<WorkloadEntity> {
    const repository = this.getRepository(bindings);
    const now = new Date();
    const entity = repository.create({
      ...workload,
      createdAt: now,
      updatedAt: now,
    });
    return await repository.save(entity);
  }

  @mapError((e) => new FindEntityError({ cause: e }))
  async list(bindings: AppBindings): Promise<WorkloadEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError({ cause: e }))
  async read(
    bindings: AppBindings,
    workloadId: string,
  ): Promise<WorkloadEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id: workloadId });
  }

  @mapError((e) => new UpdateEntityError({ cause: e }))
  async update(
    bindings: AppBindings,
    payload: UpdateWorkloadRequest,
  ): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const updated = await repository.update(
      { id: payload.id },
      {
        name: payload.name,
        description: payload.description,
        tags: payload.tags,
        updatedAt: new Date(),
      },
    );
    return updated.affected ? updated.affected > 0 : false;
  }

  @mapError((e) => new RemoveEntityError({ cause: e }))
  async remove(bindings: AppBindings, workloadId: string): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const result = await repository.delete({ id: workloadId });
    return result.affected ? result.affected > 0 : false;
  }
}

export const workloadService = new WorkloadService();
