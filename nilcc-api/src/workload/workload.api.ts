import { z } from "zod";

export const ApiRequestCreateWorkloadSchema = z.object({
  name: z.string().min(1, "Name is required"),
  description: z.string().optional(),
  tags: z.array(z.string()).optional(),
  dockerCompose: z.string().min(1, "Docker Compose is required"),
  serviceToExpose: z.string().min(1, "Service to expose is required"),
  servicePortToExpose: z
    .number()
    .int()
    .min(1, "Service port to expose is required"),
  memory: z.number().int().min(1, "Memory must be a positive integer"),
  cpu: z.number().int().min(1, "CPU must be a positive integer"),
});
export type CreateWorkloadRequest = z.infer<
  typeof ApiRequestCreateWorkloadSchema
>;

export const ApiResponseCreateWorkloadSchema =
  ApiRequestCreateWorkloadSchema.extend({
    id: z.string().uuid(),
    status: z.enum(["pending", "running", "stopped", "error"]),
    createdAt: z.string().datetime(),
    updatedAt: z.string().datetime(),
  });
export type CreateWorkloadResponse = z.infer<
  typeof ApiResponseCreateWorkloadSchema
>;

export const ApiResponseGetWorkloadSchema = ApiResponseCreateWorkloadSchema;
export type GetWorkloadResponse = z.infer<typeof ApiResponseGetWorkloadSchema>;

export const ApiResponseListWorkloadsSchema = z.array(
  ApiResponseGetWorkloadSchema,
);
export type ListWorkloadsResponse = z.infer<
  typeof ApiResponseListWorkloadsSchema
>;

export const ApiRequestUpdateWorkloadSchema = z.object({
  id: z.string().uuid(),
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
});
export type UpdateWorkloadRequest = z.infer<
  typeof ApiRequestUpdateWorkloadSchema
>;
