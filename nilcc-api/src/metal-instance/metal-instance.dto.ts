import { z } from "zod";
import { Uuid } from "#/common/types";

export const Resource = z.object({ reserved: z.number(), total: z.number() });
export type Resource = z.infer<typeof Resource>;

export const RegisterMetalInstanceRequest = z
  .object({
    id: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    hostname: z.string().min(1, "hostname is required"),
    memoryMb: Resource,
    cpus: Resource,
    diskSpaceGb: Resource,
    gpus: z.number().int().positive().optional(),
    gpuModel: z.string().optional(),
  })
  .openapi({ ref: "RegisterMetalInstanceRequest" });
export type RegisterMetalInstanceRequest = z.infer<
  typeof RegisterMetalInstanceRequest
>;

export const GetMetalInstanceResponse = RegisterMetalInstanceRequest.extend({
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
}).openapi({
  ref: "GetMetalInstanceResponse",
});
export type GetMetalInstanceResponse = z.infer<typeof GetMetalInstanceResponse>;

export const ListMetalInstancesResponse = z
  .array(GetMetalInstanceResponse)
  .openapi({ ref: "ListMetalInstancesResponse" });
export type ListMetalInstancesResponse = z.infer<
  typeof ListMetalInstancesResponse
>;
