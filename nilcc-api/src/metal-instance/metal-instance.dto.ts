import { z } from "zod";
import { Uuid } from "#/common/types";

export const CreateMetalInstanceRequest = z
  .object({
    hostname: z.string().min(1, "hostname is required"),
    memory: z.number().int().min(1, "Memory must be a positive integer"),
    cpu: z.number().int().min(1, "CPU must be a positive integer"),
    gpu: z.number().int().min(1, "GPU must be a positive integer").optional(),
    gpuModel: z.string().optional(),
    ipAddress: z.string().min(1, "IP Address is required"),
  })
  .openapi({ ref: "CreateMetalInstanceRequest" });
export type CreateMetalInstanceRequest = z.infer<
  typeof CreateMetalInstanceRequest
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
    hostname: z.string().min(1, "hostname is required").optional(),
    memory: z
      .number()
      .int()
      .min(1, "Memory must be a positive integer")
      .optional(),
    cpu: z.number().int().min(1, "CPU must be a positive integer").optional(),
    gpu: z.number().int().min(1, "GPU must be a positive integer").optional(),
    gpuModel: z.string().optional(),
    ipAddress: z.string().min(1, "IP Address is required").optional(),
  })
  .openapi({ ref: "UpdateMetalInstanceRequest" });
export type UpdateMetalInstanceRequest = z.infer<
  typeof UpdateMetalInstanceRequest
>;
