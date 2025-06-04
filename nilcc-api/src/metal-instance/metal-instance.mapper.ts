import type { CreateMetalInstanceResponse } from "#/metal-instance/metal-instance.dto";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

export const metalInstanceMapper = {
  entityToResponse(
    metalInstance: MetalInstanceEntity,
  ): CreateMetalInstanceResponse {
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
};
