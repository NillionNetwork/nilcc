import { z } from "zod";
import { Uuid } from "#/common/types";
import { GetWorkloadResponse } from "#/workload/workload.dto";

export const RegisterMetalInstanceRequest = z
  .object({
    id: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    hostname: z.string().min(10, "hostname is required"),
    memory: z.number().int().positive(),
    cpu: z.number().int().positive(),
    disk: z.number().int().positive(),
    gpu: z.number().int().positive().optional(),
    gpuModel: z.string().optional(),
    ipAddress: z.string().ip(),
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

export const SyncMetalInstanceRequest = z
  .object({
    id: Uuid,
  })
  .openapi({ ref: "SyncMetalInstanceRequest" });
export type SyncMetalInstanceRequest = z.infer<typeof SyncMetalInstanceRequest>;

export const SyncMetalInstanceResponse = GetMetalInstanceResponse.extend({
  workloads: GetWorkloadResponse.array(),
}).openapi({ ref: "SyncMetalInstanceResponse" });
export type SyncMetalInstanceResponse = z.infer<
  typeof SyncMetalInstanceResponse
>;
