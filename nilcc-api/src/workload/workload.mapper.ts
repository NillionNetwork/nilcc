import type { CreateWorkloadResponse } from "#/workload/workload.dto";
import type { WorkloadEntity } from "#/workload/workload.entity";

export const workloadMapper = {
  entityToResponse(workload: WorkloadEntity): CreateWorkloadResponse {
    return {
      id: workload.id,
      name: workload.name,
      description: workload.description ?? undefined,
      tags: workload.tags ?? undefined,
      dockerCompose: workload.dockerCompose,
      envVars: workload.envVars ?? undefined,
      serviceToExpose: workload.serviceToExpose,
      servicePortToExpose: workload.servicePortToExpose,
      memory: workload.memory,
      cpus: workload.cpus,
      disk: workload.disk,
      status: workload.status,
      createdAt: workload.createdAt.toISOString(),
      updatedAt: workload.updatedAt.toISOString(),
    };
  },
};
