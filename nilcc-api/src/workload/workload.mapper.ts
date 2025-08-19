import type { CreateWorkloadResponse } from "#/workload/workload.dto";
import type { WorkloadEntity } from "#/workload/workload.entity";

export const workloadMapper = {
  entityToResponse(
    workload: WorkloadEntity,
    workloadsDomain: string,
    metalInstancesDomain: string,
  ): CreateWorkloadResponse {
    const domain = workload.domain || `${workload.id}.${workloadsDomain}`;
    return {
      workloadId: workload.id,
      name: workload.name,
      dockerCompose: workload.dockerCompose,
      envVars: workload.envVars ?? undefined,
      publicContainerName: workload.serviceToExpose,
      publicContainerPort: workload.servicePortToExpose,
      memory: workload.memory,
      cpus: workload.cpus,
      gpus: workload.gpus,
      disk: workload.disk,
      status: workload.status,
      domain,
      metalInstanceDomain: `${workload.metalInstance.id}.${metalInstancesDomain}`,
      accountId: workload.account.id,
      createdAt: workload.createdAt.toISOString(),
      updatedAt: workload.updatedAt.toISOString(),
    };
  },
};
