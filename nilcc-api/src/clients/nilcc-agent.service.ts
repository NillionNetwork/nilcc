import { AgentRequestError } from "#/common/errors";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import type { WorkloadEntity } from "#/workload/workload.entity";

export interface NilccAgentClient {
  createWorkload(
    metalInstance: MetalInstanceEntity,
    workload: WorkloadEntity,
    domain: string,
  ): Promise<void>;
  deleteWorkload(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
  ): Promise<void>;
}

export class DefaultNilccAgentClient implements NilccAgentClient {
  async createWorkload(
    metalInstance: MetalInstanceEntity,
    workload: WorkloadEntity,
    domain: string,
  ): Promise<void> {
    const url = `${metalInstance.endpoint}/api/v1/workloads/create`;
    const request: CreateWorkloadRequest = {
      id: workload.id,
      dockerCompose: workload.dockerCompose,
      envVars: workload.envVars,
      externalFiles: workload.files,
      publicContainerName: workload.serviceToExpose,
      publicContainerPort: workload.servicePortToExpose,
      memoryMb: workload.memory,
      cpus: workload.cpus,
      gpus: workload.gpus,
      diskSpaceGb: workload.disk,
      domain,
    };
    await this.post(url, request, metalInstance);
  }

  async deleteWorkload(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
  ): Promise<void> {
    const url = `${metalInstance.endpoint}/api/v1/workloads/delete`;
    const request: DeleteWorkloadRequest = {
      id: workloadId,
    };
    await this.post(url, request, metalInstance);
  }

  async post(
    url: string,
    request: unknown,
    metalInstance: MetalInstanceEntity,
  ): Promise<void> {
    const response = await fetch(url, {
      method: "POST",
      body: JSON.stringify(request),
      headers: {
        Authorization: `Bearer ${metalInstance.token}`,
      },
    });
    if (!response.ok) {
      const body = await response.json();
      throw new AgentRequestError(body);
    }
  }
}

type CreateWorkloadRequest = {
  id: string;
  dockerCompose: string;
  envVars?: Record<string, string>;
  externalFiles?: Record<string, string>;
  publicContainerName: string;
  publicContainerPort: number;
  memoryMb: number;
  cpus: number;
  gpus: number;
  diskSpaceGb: number;
  domain: string;
};

type DeleteWorkloadRequest = {
  id: string;
};
