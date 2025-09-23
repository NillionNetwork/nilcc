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
    availableArtifactVersions: z.string().array(),
  })
  .openapi({ ref: "HeartbeatRequest" });
export type HeartbeatRequest = z.infer<typeof HeartbeatRequest>;

export const HeartbeatResponse = z
  .object({
    metalInstanceId: Uuid,
    expectedArtifactVersions: z.string().array(),
  })
  .openapi({ ref: "HeartbeatResponse" });
export type HeartbeatResponse = z.infer<typeof HeartbeatResponse>;

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
    availableArtifactVersions: z.string().array(),
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
