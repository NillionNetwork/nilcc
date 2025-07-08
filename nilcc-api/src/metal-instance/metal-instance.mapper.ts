import type { GetMetalInstanceResponse } from "#/metal-instance/metal-instance.dto";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

export const metalInstanceMapper = {
  entityToResponse(
    metalInstance: MetalInstanceEntity,
  ): GetMetalInstanceResponse {
    return {
      agentVersion: metalInstance.agentVersion,
      hostname: metalInstance.hostname,
      endpoint: metalInstance.endpoint,
      memoryMb: {
        total: metalInstance.totalMemory,
        reserved: metalInstance.osReservedMemory,
      },
      cpus: {
        total: metalInstance.totalCpus,
        reserved: metalInstance.osReservedCpus,
      },
      diskSpaceGb: {
        total: metalInstance.totalDisk,
        reserved: metalInstance.osReservedDisk,
      },
      id: metalInstance.id,
      gpus: metalInstance.gpus,
      gpuModel: metalInstance.gpuModel ?? undefined,
      createdAt: metalInstance.createdAt.toISOString(),
      updatedAt: metalInstance.updatedAt.toISOString(),
    };
  },
};
