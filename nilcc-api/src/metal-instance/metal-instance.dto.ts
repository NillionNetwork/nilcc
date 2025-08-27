import { z } from "zod";
import { Uuid } from "#/common/types";

export const Resource = z.object({
  reserved: z.number().nonnegative(),
  total: z.number().nonnegative(),
});
export type Resource = z.infer<typeof Resource>;

export const RegisterMetalInstanceRequest = z
  .object({
    metalInstanceId: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    publicIp: z.string().ip({ version: "v4" }),
    token: z.string(),
    hostname: z.string().min(1, "hostname is required"),
    memoryMb: Resource,
    cpus: Resource,
    diskSpaceGb: Resource,
    gpus: z.number().nonnegative(),
    gpuModel: z.string().optional(),
  })
  .openapi({ ref: "RegisterMetalInstanceRequest" });
export type RegisterMetalInstanceRequest = z.infer<
  typeof RegisterMetalInstanceRequest
>;

export const HeartbeatRequest = z
  .object({
    metalInstanceId: Uuid,
  })
  .openapi({ ref: "HeartbeatRequest" });
export type HeartbeatRequest = z.infer<typeof HeartbeatRequest>;

export const GetMetalInstanceResponse = z
  .object({
    metalInstanceId: Uuid,
    agentVersion: z.string(),
    hostname: z.string(),
    publicIp: z.string(),
    memoryMb: Resource,
    cpus: Resource,
    diskSpaceGb: Resource,
    gpus: z.number(),
    gpuModel: z.string().optional(),
    createdAt: z.string().datetime(),
    updatedAt: z.string().datetime(),
    lastSeenAt: z.string().datetime(),
  })
  .openapi({
    ref: "GetMetalInstanceResponse",
  });
export type GetMetalInstanceResponse = z.infer<typeof GetMetalInstanceResponse>;

export const ListMetalInstancesResponse = z
  .array(GetMetalInstanceResponse)
  .openapi({ ref: "ListMetalInstancesResponse" });
export type ListMetalInstancesResponse = z.infer<
  typeof ListMetalInstancesResponse
>;

export const DeleteMetalInstanceRequest = z
  .object({
    metalInstanceId: Uuid,
  })
  .openapi({
    ref: "DeleteMetalInstanceRequest",
  });
export type DeleteMetalInstanceRequest = z.infer<
  typeof DeleteMetalInstanceRequest
>;

export const WorkloadEventKind = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("created") }),
  z.object({ kind: z.literal("starting") }),
  z.object({ kind: z.literal("stopped") }),
  z.object({ kind: z.literal("vmRestarted") }),
  z.object({ kind: z.literal("forcedRestart") }),
  z.object({ kind: z.literal("awaitingCert") }),
  z.object({ kind: z.literal("running") }),
  z.object({ kind: z.literal("failedToStart"), error: z.string() }),
]);
export type WorkloadEventKind = z.infer<typeof WorkloadEventKind>;

export const SubmitEventRequest = z
  .object({
    metalInstanceId: Uuid,
    workloadId: Uuid,
    event: WorkloadEventKind,
  })
  .openapi({ ref: "SubmitEventRequest" });
export type SubmitEventRequest = z.infer<typeof SubmitEventRequest>;
