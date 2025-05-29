import { z } from "zod";

export const PathSchema = z
  .string()
  .startsWith("/")
  .regex(/^(\/[a-z0-9_:.-]+)+$/i, {
    message: "Path must follow format: /parent/child/:param/grandchild",
  })
  .brand<"path">();

export type Path = z.infer<typeof PathSchema>;

export const PathsV1 = {
  docs: PathSchema.parse("/api/v1/openapi/docs"),
  workload: {
    create: PathSchema.parse("/api/v1/workloads"),
    list: PathSchema.parse("/api/v1/workloads"),
    read: PathSchema.parse("/api/v1/workloads/:id"),
    update: PathSchema.parse("/api/v1/workloads"),
    remove: PathSchema.parse("/api/v1/workloads/:id"),
  },
} as const;
