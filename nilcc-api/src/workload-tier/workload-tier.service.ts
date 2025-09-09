import type { QueryRunner, Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import { EntityAlreadyExists, isUniqueConstraint } from "#/common/errors";
import type { AppBindings } from "#/env";
import type { CreateWorkloadTierRequest } from "./workload-tier.dto";
import { WorkloadTierEntity } from "./workload-tier.entity";

export class WorkloadTierService {
  getRepository(
    bindings: AppBindings,
    tx?: QueryRunner,
  ): Repository<WorkloadTierEntity> {
    if (tx) {
      return tx.manager.getRepository(WorkloadTierEntity);
    }
    return bindings.dataSource.getRepository(WorkloadTierEntity);
  }

  async create(
    bindings: AppBindings,
    request: CreateWorkloadTierRequest,
  ): Promise<WorkloadTierEntity> {
    const repository = this.getRepository(bindings);
    try {
      return await repository.save({
        id: uuidv4(),
        name: request.name,
        cpus: request.cpus,
        gpus: request.gpus,
        memory: request.memoryMb,
        disk: request.diskGb,
        cost: request.cost,
      });
    } catch (e: unknown) {
      if (isUniqueConstraint(e)) {
        throw new EntityAlreadyExists("workload tier");
      }
      throw e;
    }
  }

  async remove(bindings: AppBindings, id: string): Promise<void> {
    const repository = this.getRepository(bindings);
    await repository.delete({ id });
  }

  async list(bindings: AppBindings): Promise<WorkloadTierEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }
}
