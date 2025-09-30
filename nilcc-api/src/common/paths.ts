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
  account: {
    create: PathSchema.parse("/api/v1/accounts/create"),
    update: PathSchema.parse("/api/v1/accounts/update"),
    list: PathSchema.parse("/api/v1/accounts/list"),
    read: PathSchema.parse("/api/v1/accounts/:id"),
    me: PathSchema.parse("/api/v1/accounts/me"),
    addCredits: PathSchema.parse("/api/v1/accounts/add-credits"),
  },
  artifacts: {
    enable: PathSchema.parse("/api/v1/artifacts/enable"),
    list: PathSchema.parse("/api/v1/artifacts/list"),
    disable: PathSchema.parse("/api/v1/artifacts/disable"),
  },
  workload: {
    create: PathSchema.parse("/api/v1/workloads/create"),
    list: PathSchema.parse("/api/v1/workloads/list"),
    read: PathSchema.parse("/api/v1/workloads/:id"),
    delete: PathSchema.parse("/api/v1/workloads/delete"),
    logs: PathSchema.parse("/api/v1/workloads/logs"),
    stats: PathSchema.parse("/api/v1/workloads/stats"),
    restart: PathSchema.parse("/api/v1/workloads/restart"),
    start: PathSchema.parse("/api/v1/workloads/start"),
    stop: PathSchema.parse("/api/v1/workloads/stop"),
  },
  workloadContainers: {
    list: PathSchema.parse("/api/v1/workload-containers/list"),
    logs: PathSchema.parse("/api/v1/workload-containers/logs"),
  },
  workloadEvents: {
    submit: PathSchema.parse("/api/v1/workload-events/submit"),
    list: PathSchema.parse("/api/v1/workload-events/list"),
  },
  workloadTiers: {
    create: PathSchema.parse("/api/v1/workload-tiers/create"),
    list: PathSchema.parse("/api/v1/workload-tiers/list"),
    delete: PathSchema.parse("/api/v1/workload-tiers/delete"),
  },
  metalInstance: {
    register: PathSchema.parse("/api/v1/metal-instances/register"),
    heartbeat: PathSchema.parse("/api/v1/metal-instances/heartbeat"),
    read: PathSchema.parse("/api/v1/metal-instances/:id"),
    list: PathSchema.parse("/api/v1/metal-instances/list"),
    delete: PathSchema.parse("/api/v1/metal-instances/delete"),
  },
  system: {
    health: PathSchema.parse("/health"),
  },
} as const;
