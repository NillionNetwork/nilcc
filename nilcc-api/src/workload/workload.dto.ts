import { z } from "zod";
import { Uuid } from "#/common/types";

export const CreateWorkloadRequest = z
  .object({
    name: z.string().min(1, "Name is required"),
    description: z.string().optional(),
    tags: z.array(z.string()).optional(),
    dockerCompose: z.string(),
    envVars: z.record(z.string(), z.string()).optional(),
    serviceToExpose: z.string().min(1, "Service to expose is required"),
    servicePortToExpose: z.number().int().positive(),
    memory: z.number().int().positive(),
    cpus: z.number().int().positive(),
    disk: z
      .number()
      .int()
      .min(10, "Disk must be at least 10GB")
      .max(100, "Disk must be at most 100GB"),
    gpus: z.number().int(),
  })
  .openapi({ ref: "CreateWorkloadRequest" });
export type CreateWorkloadRequest = z.infer<typeof CreateWorkloadRequest>;

export const CreateWorkloadResponse = CreateWorkloadRequest.extend({
  id: Uuid,
  status: z.enum(["scheduled", "running", "stopped", "error"]),
  createdAt: z.string().datetime(),
  updatedAt: z.string().datetime(),
}).openapi({ ref: "CreateWorkloadResponse" });
export type CreateWorkloadResponse = z.infer<typeof CreateWorkloadResponse>;

export const GetWorkloadResponse = CreateWorkloadResponse.openapi({
  ref: "GetWorkloadResponse",
});
export type GetWorkloadResponse = z.infer<typeof GetWorkloadResponse>;

export const ListWorkloadsResponse = z
  .array(GetWorkloadResponse)
  .openapi({ ref: "ListWorkloadsResponse" });
export type ListWorkloadsResponse = z.infer<typeof ListWorkloadsResponse>;
