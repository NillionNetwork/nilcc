import type { Repository } from "typeorm";
import {
  CreateOrUpdateEntityError,
  FindEntityError,
  GetRepositoryError,
  mapError,
  RemoveEntityError,
} from "#/common/errors";
import type { AppBindings } from "#/env";
import type { RegisterMetalInstanceRequest } from "./metal-instance.dto";
import { MetalInstanceEntity } from "./metal-instance.entity";

export class MetalInstanceService {
  @mapError((e) => new GetRepositoryError({ cause: e }))
  getRepository(bindings: AppBindings): Repository<MetalInstanceEntity> {
    return bindings.dataSource.getRepository(MetalInstanceEntity);
  }

  @mapError((e) => new FindEntityError({ cause: e }))
  async list(bindings: AppBindings): Promise<MetalInstanceEntity[]> {
    const repository = this.getRepository(bindings);
    return await repository.find();
  }

  @mapError((e) => new FindEntityError({ cause: e }))
  async read(
    bindings: AppBindings,
    workloadId: string,
  ): Promise<MetalInstanceEntity | null> {
    const repository = this.getRepository(bindings);
    return await repository.findOneBy({ id: workloadId });
  }

  @mapError((e) => new RemoveEntityError({ cause: e }))
  async remove(bindings: AppBindings, workloadId: string): Promise<boolean> {
    const repository = this.getRepository(bindings);
    const result = await repository.delete({ id: workloadId });
    return result.affected ? result.affected > 0 : false;
  }

  @mapError((e) => new CreateOrUpdateEntityError({ cause: e }))
  async createOrUpdate(
    bindings: AppBindings,
    metalInstance: RegisterMetalInstanceRequest,
  ) {
    const repository = this.getRepository(bindings);
    const now = new Date();
    await repository.upsert(
      {
        id: metalInstance.id,
        agentVersion: metalInstance.agentVersion,
        hostname: metalInstance.hostname,
        memory: metalInstance.memory,
        cpu: metalInstance.cpu,
        disk: metalInstance.disk,
        gpu: metalInstance.gpu,
        gpuModel: metalInstance.gpuModel,
        ipAddress: metalInstance.ipAddress,
        createdAt: now,
        updatedAt: now,
      },
      ["id"],
    );
  }
}

export const metalInstanceService = new MetalInstanceService();
