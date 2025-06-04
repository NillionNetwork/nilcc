import { z } from "zod";
import { Uuid } from "#/common/types";

export const CreateMetalInstanceRequest = z
  .object({
    agentVersion: z.string().min(1, "Agent version is required"),
    hostname: z.string().min(10, "hostname is required"),
    memory: z.number().int().positive(),
    cpu: z.number().int().positive(),
    disk: z.number().int().positive(),
    gpu: z.number().int().positive().optional(),
    gpuModel: z.string().optional(),
    ipAddress: z.string().ip(),
  })
  .openapi({ ref: "CreateMetalInstanceRequest" });
export type CreateMetalInstanceRequest = z.infer<
  typeof CreateMetalInstanceRequest
>;

export const RegisterMetalInstanceRequest = CreateMetalInstanceRequest.extend({
  id: Uuid,
}).openapi({ ref: "RegisterMetalInstanceRequest" });
export type RegisterMetalInstanceRequest = z.infer<
  typeof RegisterMetalInstanceRequest
>;

export const CreateMetalInstanceResponse = CreateMetalInstanceRequest.extend({
  id: Uuid,
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
}).openapi({ ref: "CreateMetalInstanceResponse" });
export type CreateMetalInstanceResponse = z.infer<
  typeof CreateMetalInstanceResponse
>;

export const GetMetalInstanceResponse = CreateMetalInstanceResponse.openapi({
  ref: "GetMetalInstanceResponse",
});
export type GetMetalInstanceResponse = z.infer<typeof GetMetalInstanceResponse>;

export const ListMetalInstancesResponse = z
  .array(GetMetalInstanceResponse)
  .openapi({ ref: "ListMetalInstancesResponse" });
export type ListMetalInstancesResponse = z.infer<
  typeof ListMetalInstancesResponse
>;

export const UpdateMetalInstanceRequest = z
  .object({
    id: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    hostname: z.string().min(10, "hostname is required").optional(),
    memory: z.number().int().positive().optional(),
    cpu: z.number().int().positive().optional(),
    gpu: z.number().int().positive().optional(),
    gpuModel: z.string().optional(),
    ipAddress: z.string().ip().optional(),
  })
  .openapi({ ref: "UpdateMetalInstanceRequest" });
export type UpdateMetalInstanceRequest = z.infer<
  typeof UpdateMetalInstanceRequest
>;
