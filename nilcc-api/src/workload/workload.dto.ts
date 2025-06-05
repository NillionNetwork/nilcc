import { z } from "zod";
import { Uuid } from "#/common/types";

export const CreateWorkloadRequest = z
  .object({
    name: z.string().min(1, "Name is required"),
    description: z.string().optional(),
    tags: z.array(z.string()).optional(),
    dockerCompose: z.string().min(1, "Docker Compose is required"),
    serviceToExpose: z.string().min(1, "Service to expose is required"),
    servicePortToExpose: z.number().int().positive(),
    memory: z.number().int().positive(),
    cpu: z.number().int().positive(),
    disk: z
      .number()
      .int()
      .min(10, "Disk must be at least 10GB")
      .max(100, "Disk must be at most 100GB"),
    gpu: z.number().int().positive().optional(),
  })
  .openapi({ ref: "CreateWorkloadRequest" });
export type CreateWorkloadRequest = z.infer<typeof CreateWorkloadRequest>;

export const CreateWorkloadResponse = CreateWorkloadRequest.extend({
  id: Uuid,
  status: z.enum(["pending", "running", "stopped", "error"]),
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

export const UpdateWorkloadRequest = z
  .object({
    id: Uuid,
    name: z.string().min(1, "Name is required").optional(),
    description: z.string().optional(),
    tags: z.array(z.string()).optional(),
    dockerCompose: z.string().min(1, "Docker Compose is required").optional(),
    serviceToExpose: z
      .string()
      .min(1, "Service to expose is required")
      .optional(),
    servicePortToExpose: z
      .number()
      .int()
      .min(1, "Service port to expose is required")
      .optional(),
    memory: z
      .number()
      .int()
      .min(1, "Memory must be a positive integer")
      .optional(),
    cpu: z.number().int().min(1, "CPU must be a positive integer").optional(),
  })
  .openapi({ ref: "UpdateWorkloadRequest" });
export type UpdateWorkloadRequest = z.infer<typeof UpdateWorkloadRequest>;
