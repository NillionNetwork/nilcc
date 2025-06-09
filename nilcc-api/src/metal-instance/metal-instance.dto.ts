import { z } from "zod";
import { Uuid } from "#/common/types";
import { GetWorkloadResponse } from "#/workload/workload.dto";

export const RegisterMetalInstanceRequest = z
  .object({
    id: Uuid,
    agentVersion: z.string().min(1, "Agent version is required"),
    hostname: z.string().min(10, "hostname is required"),
    totalMemory: z.number().int().positive(),
    osReservedMemory: z.number().int().positive(),
    totalCpu: z.number().int().positive(),
    osReservedCpu: z.number().int().positive(),
    totalDisk: z.number().int().positive(),
    osReservedDisk: z.number().int().positive(),
    gpu: z.number().int().positive().optional(),
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

export const SyncWorkload = z
  .object({
    id: Uuid,
    status: z.enum(["pending", "running", "stopped", "error"]),
  })
  .array();

export const SyncMetalInstanceRequest = z
  .object({
    id: Uuid,
    workloads: SyncWorkload,
  })
  .openapi({ ref: "SyncMetalInstanceRequest" });
export type SyncMetalInstanceRequest = z.infer<typeof SyncMetalInstanceRequest>;

export const SyncMetalInstanceResponse = GetMetalInstanceResponse.extend({
  workloads: GetWorkloadResponse.array(),
}).openapi({ ref: "SyncMetalInstanceResponse" });
export type SyncMetalInstanceResponse = z.infer<
  typeof SyncMetalInstanceResponse
>;
