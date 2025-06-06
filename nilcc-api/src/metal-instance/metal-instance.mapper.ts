import type {
  GetMetalInstanceResponse,
  SyncMetalInstanceResponse,
} from "#/metal-instance/metal-instance.dto";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import { workloadMapper } from "#/workload/workload.mapper";

export const metalInstanceMapper = {
  entityToResponse(
    metalInstance: MetalInstanceEntity,
  ): GetMetalInstanceResponse {
    return {
      agentVersion: metalInstance.agentVersion,
      hostname: metalInstance.hostname,
      memory: metalInstance.memory,
      cpu: metalInstance.cpu,
      disk: metalInstance.disk,
      ipAddress: metalInstance.ipAddress,
      id: metalInstance.id,
      gpu: metalInstance.gpu ?? undefined,
      gpuModel: metalInstance.gpuModel ?? undefined,
      createdAt: metalInstance.createdAt.toISOString(),
      updatedAt: metalInstance.updatedAt.toISOString(),
    };
  },

  syncEntityToResponse(
    metalInstance: MetalInstanceEntity,
  ): SyncMetalInstanceResponse {
    const metalInstanceResponse = this.entityToResponse(metalInstance);
    const workloads = metalInstance.workloads
      ? metalInstance.workloads.map((w) => workloadMapper.entityToResponse(w))
      : [];
    return {
      ...metalInstanceResponse,
      workloads,
    };
  },
};
