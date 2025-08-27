import type { GetMetalInstanceResponse } from "#/metal-instance/metal-instance.dto";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

export const metalInstanceMapper = {
  entityToResponse(
    metalInstance: MetalInstanceEntity,
  ): GetMetalInstanceResponse {
    return {
      agentVersion: metalInstance.agentVersion,
      hostname: metalInstance.hostname,
      publicIp: metalInstance.publicIp,
      memoryMb: {
        total: metalInstance.totalMemory,
        reserved: metalInstance.reservedMemory,
      },
      cpus: {
        total: metalInstance.totalCpus,
        reserved: metalInstance.reservedCpus,
      },
      diskSpaceGb: {
        total: metalInstance.totalDisk,
        reserved: metalInstance.reservedDisk,
      },
      metalInstanceId: metalInstance.id,
      gpus: metalInstance.gpus,
      gpuModel: metalInstance.gpuModel ?? undefined,
      createdAt: metalInstance.createdAt.toISOString(),
      updatedAt: metalInstance.updatedAt.toISOString(),
      lastSeenAt: metalInstance.lastSeenAt.toISOString(),
    };
  },
};
