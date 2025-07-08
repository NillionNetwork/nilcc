import { z } from "zod";
import { Uuid } from "#/common/types";

export const Resource = z.object({
  reserved: z.number().nonnegative(),
  total: z.number().nonnegative(),
});
export type Resource = z.infer<typeof Resource>;

export const RegisterMetalInstanceRequest = z
  .object({
    id: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    endpoint: z.string(),
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

export const GetMetalInstanceResponse = z
  .object({
    id: Uuid,
    agentVersion: z.string(),
    endpoint: z.string(),
    hostname: z.string(),
    memoryMb: Resource,
    cpus: Resource,
    diskSpaceGb: Resource,
    gpus: z.number(),
    gpuModel: z.string().optional(),
    createdAt: z.string().datetime(),
    updatedAt: z.string().datetime(),
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
