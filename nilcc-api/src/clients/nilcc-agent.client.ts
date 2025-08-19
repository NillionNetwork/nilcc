import type { Logger } from "pino";
import z from "zod";
import { AgentCreateWorkloadError, AgentRequestError } from "#/common/errors";
import type { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";
import type { DockerCredentials } from "#/workload/workload.dto";
import type { WorkloadEntity } from "#/workload/workload.entity";

const ALLOWED_CREATE_WORKLOAD_ERRORS: string[] = [
  "DOCKER_COMPOSE",
  "DOMAIN_EXISTS",
  "AGENT_DOMAIN",
  "RESOURCE_LIMIT",
];

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

  containers(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
  ): Promise<Array<Container>>;

  containerLogs(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
    request: ContainerLogsRequest,
  ): Promise<Array<string>>;

  systemLogs(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
    request: SystemLogsRequest,
  ): Promise<Array<string>>;
}

export class DefaultNilccAgentClient implements NilccAgentClient {
  protected scheme: string;
  protected subdomain: string;
  protected port: number;
  protected log: Logger;

  constructor(
    scheme: "http" | "https",
    subdomain: string,
    port: number,
    log: Logger,
  ) {
    this.scheme = scheme;
    this.subdomain = subdomain;
    this.port = port;
    this.log = log;
  }

  makeUrl(metalInstance: MetalInstanceEntity, path: string) {
    return `${this.scheme}://${metalInstance.id}.${this.subdomain}:${this.port}${path}`;
  }

  async createWorkload(
    metalInstance: MetalInstanceEntity,
    workload: WorkloadEntity,
    domain: string,
  ): Promise<void> {
    const url = this.makeUrl(metalInstance, "/api/v1/workloads/create");
    this.log.info(
      `Creating workload ${workload.id} in agent ${metalInstance.id}`,
    );
    const request: CreateWorkloadRequest = {
      id: workload.id,
      dockerCompose: workload.dockerCompose,
      envVars: workload.envVars,
      files: workload.files,
      dockerCredentials: workload.dockerCredentials,
      publicContainerName: workload.serviceToExpose,
      publicContainerPort: workload.servicePortToExpose,
      memoryMb: workload.memory,
      cpus: workload.cpus,
      gpus: workload.gpus,
      diskSpaceGb: workload.disk,
      domain,
    };
    try {
      await this.post(url, request, metalInstance);
    } catch (error: unknown) {
      if (
        error instanceof AgentRequestError &&
        ALLOWED_CREATE_WORKLOAD_ERRORS.includes(error.agentErrorKind)
      ) {
        throw new AgentCreateWorkloadError(
          error.agentErrorKind,
          error.agentErrorDescription,
        );
      }
      throw error;
    }
  }

  async deleteWorkload(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
  ): Promise<void> {
    const url = this.makeUrl(metalInstance, "/api/v1/workloads/delete");

    const request: DeleteWorkloadRequest = {
      id: workloadId,
    };
    try {
      this.log.info(
        `Deleting workload ${workloadId} in agent ${metalInstance.id}`,
      );
      await this.post(url, request, metalInstance);
    } catch (e: unknown) {
      if (
        e instanceof AgentRequestError &&
        e.agentErrorKind === "WORKLOAD_NOT_FOUND"
      ) {
        this.log.warn(
          `Attempted to delete workload ${workloadId} in agent ${metalInstance.id} but it didn't exist`,
        );
        return;
      }
      throw e;
    }
  }

  async containers(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
  ): Promise<Container[]> {
    const url = this.makeUrl(
      metalInstance,
      `/api/v1/workloads/${workloadId}/containers/list`,
    );
    this.log.info(
      `Looking up containers for workload ${workloadId} in agent ${metalInstance.id}`,
    );
    return await this.get(url, metalInstance, Container.array());
  }

  async containerLogs(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
    request: ContainerLogsRequest,
  ): Promise<string[]> {
    const params: [string, string][] = Object.entries(request).map(
      ([key, value]) => [String(key), String(value)],
    );
    const queryParams = new URLSearchParams(params).toString();
    const url = this.makeUrl(
      metalInstance,
      `/api/v1/workloads/${workloadId}/containers/logs?${queryParams}`,
    );
    this.log.info(
      `Looking up container logs for workload ${workloadId} in agent ${metalInstance.id}`,
    );
    const response = await this.get(url, metalInstance, LogsResponse);
    return response.lines;
  }

  async systemLogs(
    metalInstance: MetalInstanceEntity,
    workloadId: string,
    request: SystemLogsRequest,
  ): Promise<string[]> {
    const params: [string, string][] = Object.entries(request).map(
      ([key, value]) => [String(key), String(value)],
    );
    const queryParams = new URLSearchParams(params).toString();
    const url = this.makeUrl(
      metalInstance,
      `/api/v1/workloads/${workloadId}/system/logs?${queryParams}`,
    );
    this.log.info(
      `Looking up system logs for workload ${workloadId} in agent ${metalInstance.id}`,
    );
    const response = await this.get(url, metalInstance, LogsResponse);
    return response.lines;
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
        Accept: "application/json",
        "Content-Type": "application/json",
      },
    });
    if (!response.ok) {
      const body = AgentErrorDetails.parse(await response.json());
      throw new AgentRequestError(body.errorCode, body.message);
    }
  }

  async get<T extends z.ZodTypeAny>(
    url: string,
    metalInstance: MetalInstanceEntity,
    schema: T,
  ): Promise<z.infer<T>> {
    const response = await fetch(url, {
      method: "GET",
      headers: {
        Authorization: `Bearer ${metalInstance.token}`,
      },
    });
    const body = await response.json();
    if (!response.ok) {
      const error = AgentErrorDetails.parse(body);
      throw new AgentRequestError(error.errorCode, error.message);
    }
    return schema.parse(body) as z.infer<T>;
  }
}

type CreateWorkloadRequest = {
  id: string;
  dockerCompose: string;
  envVars?: Record<string, string>;
  files?: Record<string, string>;
  dockerCredentials?: DockerCredentials[];
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

export const Container = z
  .object({
    names: z.array(z.string()).openapi({
      description: "The name(s) for this container.",
      examples: [["cvm-api-1"]],
    }),
    image: z.string().openapi({
      description: "The docker image this container is using.",
      examples: ["ghcr.io/nillionnetwork/nilcc-attester:latest"],
    }),
    imageId: z.string().openapi({
      description: "The docker image identifier being used.",
      examples: [
        "sha256:a16bb0e1a3fa23179888246671ce3db9c9006030cc91b7377972d5e35a121556",
      ],
    }),
    state: z.string().openapi({
      description: "The state of this container.",
      examples: ["created"],
    }),
  })
  .openapi({
    ref: "Container",
    description: "A container running in a workload.",
  });

export type Container = z.infer<typeof Container>;

export const ContainerLogsRequest = z.object({
  container: z.string().openapi({ description: "The name of the container." }),
  tail: z.boolean().openapi({
    description:
      "Whether to get logs from the tail of the log instead of the head.",
  }),
  stream: z
    .enum(["stdout", "stderr"])
    .openapi({ description: "The stream to get logs from." }),
  maxLines: z
    .number()
    .int()
    .max(1000)
    .default(1000)
    .openapi({ description: "The maximum number of lines to get." }),
});
export type ContainerLogsRequest = z.infer<typeof ContainerLogsRequest>;

export const SystemLogsRequest = z.object({
  tail: z.boolean().openapi({
    description:
      "Whether to get logs from the tail of the log instead of the head.",
  }),
  source: z
    .enum(["cvm-agent"])
    .default("cvm-agent")
    .openapi({ description: "The source to get logs from." }),
  maxLines: z
    .number()
    .int()
    .max(1000)
    .default(1000)
    .openapi({ description: "The maximum number of lines to get." }),
});
export type SystemLogsRequest = z.infer<typeof SystemLogsRequest>;

const LogsResponse = z.object({ lines: z.string().array() });

const AgentErrorDetails = z.object({
  errorCode: z.string(),
  message: z.string(),
});
