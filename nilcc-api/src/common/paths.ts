import { z } from "zod";

export const PathSchema = z
  .string()
  .startsWith("/")
  .regex(/^(\/[a-z0-9_~:.-]+)+$/i, {
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
    events: {
      submit: PathSchema.parse("/api/v1/workloads/~/events/submit"),
    },
    containers: {
      list: PathSchema.parse("/api/v1/workloads/~/containers/list"),
      logs: PathSchema.parse("/api/v1/workloads/~/containers/logs"),
    },
  },
  metalInstance: {
    register: PathSchema.parse("/api/v1/metal-instances/~/register"),
    heartbeat: PathSchema.parse("/api/v1/metal-instances/~/heartbeat"),
    read: PathSchema.parse("/api/v1/metal-instances/:id"),
    list: PathSchema.parse("/api/v1/metal-instances"),
  },
} as const;
