import type { GetMetalInstanceResponse } from "#/metal-instance/metal-instance.dto";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

export const metalInstanceMapper = {
  entityToResponse(
    metalInstance: MetalInstanceEntity,
    subdomain: string,
  ): GetMetalInstanceResponse {
    return {
      agentVersion: metalInstance.agentVersion,
      hostname: metalInstance.hostname,
      domain: `${metalInstance.id}.${subdomain}`,
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
      availableArtifactVersions: metalInstance.availableArtifactVersions,
      createdAt: metalInstance.createdAt.toISOString(),
      updatedAt: metalInstance.updatedAt.toISOString(),
      lastSeenAt: metalInstance.lastSeenAt.toISOString(),
    };
  },
};
