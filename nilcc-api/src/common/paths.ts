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
  docs: PathSchema.parse("/openapi.json"),
  workload: {
    create: PathSchema.parse("/api/v1/workloads/create"),
    list: PathSchema.parse("/api/v1/workloads/list"),
    read: PathSchema.parse("/api/v1/workloads/:id"),
    delete: PathSchema.parse("/api/v1/workloads/delete"),
  },
  workloadContainers: {
    list: PathSchema.parse("/api/v1/workload-containers/list"),
    logs: PathSchema.parse("/api/v1/workload-containers/logs"),
  },
  workloadEvents: {
    submit: PathSchema.parse("/api/v1/workload-events/submit"),
    list: PathSchema.parse("/api/v1/workload-events/list"),
  },
  metalInstance: {
    register: PathSchema.parse("/api/v1/metal-instances/register"),
    heartbeat: PathSchema.parse("/api/v1/metal-instances/heartbeat"),
    read: PathSchema.parse("/api/v1/metal-instances/:id"),
    list: PathSchema.parse("/api/v1/metal-instances/list"),
  },
  system: {
    health: PathSchema.parse("/health"),
  },
} as const;
