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
      artifactsVersion: workload.artifactsVersion,
      dockerCompose: workload.dockerCompose,
      envVars: workload.envVars ?? undefined,
      dockerCredentials: workload.dockerCredentials ?? undefined,
      files: workload.files ?? undefined,
      publicContainerName: workload.publicContainerName,
      publicContainerPort: workload.publicContainerPort,
      memory: workload.memory,
      cpus: workload.cpus,
      gpus: workload.gpus,
      disk: workload.disk,
      creditRate: workload.creditRate,
      status: workload.status,
      domain,
      metalInstanceDomain: `${workload.metalInstance.id}.${metalInstancesDomain}`,
      accountId: workload.account.id,
      createdAt: workload.createdAt.toISOString(),
      updatedAt: workload.updatedAt.toISOString(),
    };
  },
};
