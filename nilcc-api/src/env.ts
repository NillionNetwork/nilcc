import type { Logger } from "pino";
import type { DataSource, QueryRunner } from "typeorm";
import { z } from "zod";
import { createLogger } from "#/common/logger";
import { buildDataSource } from "#/data-source";
import { MetalInstanceService } from "#/metal-instance/metal-instance.service";
import { WorkloadService } from "#/workload/workload.service";
import {
  DefaultNilccAgentClient,
  type NilccAgentClient,
} from "./clients/nilcc-agent.client";
import {
  type DnsService,
  LocalStackDnsService,
  Route53DnsService,
} from "./dns/dns.service";
export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;

export const FeatureFlag = {
  OPENAPI_SPEC: "openapi",
  PROMETHEUS_METRICS: "prometheus-metrics",
  MIGRATIONS: "migrations",
  LOCALSTACK: "localstack",
  PRETTY_LOGS: "pretty-logs",
  HTTP_ERROR_STACKTRACE: "http-error-stacktrace",
} as const;

export type FeatureFlag = (typeof FeatureFlag)[keyof typeof FeatureFlag];

export function hasFeatureFlag(
  enabledFeatures: string[],
  flag: FeatureFlag,
): boolean {
  return enabledFeatures.includes(flag);
}

export type AppEnv = {
  Bindings: AppBindings;
  Variables: AppVariables;
};

export type AppVariables = {
  txQueryRunner: QueryRunner;
};

export type DnsServices = {
  metalInstances: DnsService;
  workloads: DnsService;
};

export type AppServices = {
  metalInstance: MetalInstanceService;
  workload: WorkloadService;
  dns: DnsServices;
  nilccAgentClient: NilccAgentClient;
};

export type AppBindings = {
  config: EnvVars;
  dataSource: DataSource;
  services: AppServices;
  log: Logger;
};

export const EnvVarsSchema = z.object({
  dbUri: z.string().startsWith("postgres://"),
  enabledFeatures: z
    .string()
    .transform((d) => d.split(",").map((e) => e.trim())),
  logLevel: z.enum(LOG_LEVELS),
  metricsPort: z.number().int().positive(),
  httpApiPort: z.number().int().positive(),
  metalInstanceApiKey: z.string(),
  userApiKey: z.string(),
  workloadsDnsDomain: z.string(),
  workloadsDnsZone: z.string(),
  metalInstancesDnsDomain: z.string(),
  metalInstancesDnsZone: z.string(),
  metalInstancesEndpointScheme: z.enum(["http", "https"]).default("https"),
  metalInstancesEndpointPort: z.number().default(443),
  metalInstancesIdleThresholdSeconds: z.number().default(120),
});

export type EnvVars = z.infer<typeof EnvVarsSchema>;

// Use interface merging to define expected app vars
declare global {
  namespace NodeJS {
    interface ProcessEnv {
      APP_DB_URI: string;
      APP_ENABLED_FEATURES: string;
      APP_LOG_LEVEL: string;
      APP_METRICS_PORT: string;
      APP_HTTP_API_PORT: string;
      APP_METAL_INSTANCE_API_KEY: string;
      APP_USER_API_KEY: string;
      APP_WORKLOADS_DNS_ZONE: string;
      APP_WORKLOADS_DNS_DOMAIN: string;
      APP_METAL_INSTANCES_DNS_DOMAIN: string;
      APP_METAL_INSTANCES_DNS_ZONE: string;
      APP_METAL_INSTANCES_ENDPOINT_SCHEME?: string;
      APP_METAL_INSTANCES_ENDPOINT_PORT?: string;
      APP_METAL_INSTANCES_IDLE_THRESHOLD_SECONDS?: string;
    }
  }
}

export async function loadBindings(
  overrides: Partial<EnvVars> = {},
): Promise<AppBindings> {
  const config = parseConfigFromEnv(overrides);
  const log = createLogger(
    config.logLevel,
    hasFeatureFlag(config.enabledFeatures, FeatureFlag.PRETTY_LOGS),
  );

  const dataSource = await buildDataSource(config, log);

  const services = await buildServices(config, log);

  return {
    config,
    dataSource,
    services,
    log,
  };
}

async function buildServices(
  config: EnvVars,
  log: Logger,
): Promise<AppServices> {
  const dns = {
    workloads: await createDnsService(
      config.workloadsDnsZone,
      config.workloadsDnsDomain,
      config,
      log,
    ),
    metalInstances: await createDnsService(
      config.metalInstancesDnsZone,
      config.metalInstancesDnsDomain,
      config,
      log,
    ),
  };
  log.debug("Using DNS service: %s", dns.workloads.constructor.name);

  const metalInstanceService = new MetalInstanceService();
  const workloadService = new WorkloadService();
  const nilccAgentClient = new DefaultNilccAgentClient(
    config.metalInstancesEndpointScheme,
    config.metalInstancesDnsDomain,
    config.metalInstancesEndpointPort,
    log,
  );

  return {
    metalInstance: metalInstanceService,
    workload: workloadService,
    dns,
    nilccAgentClient,
  };
}

export function parseConfigFromEnv(overrides: Partial<EnvVars>): EnvVars {
  const tryNumber = (n: string | undefined) =>
    n !== undefined ? Number(n) : undefined;
  const config = EnvVarsSchema.parse({
    dbUri: process.env.APP_DB_URI,
    enabledFeatures: process.env.APP_ENABLED_FEATURES,
    logLevel: process.env.APP_LOG_LEVEL,
    metricsPort: Number(process.env.APP_METRICS_PORT),
    httpApiPort: Number(process.env.APP_HTTP_API_PORT),
    metalInstanceApiKey: process.env.APP_METAL_INSTANCE_API_KEY,
    userApiKey: process.env.APP_USER_API_KEY,
    workloadsDnsDomain: process.env.APP_WORKLOADS_DNS_DOMAIN,
    workloadsDnsZone: process.env.APP_WORKLOADS_DNS_ZONE,
    metalInstancesDnsZone: process.env.APP_METAL_INSTANCES_DNS_ZONE,
    metalInstancesDnsDomain: process.env.APP_METAL_INSTANCES_DNS_DOMAIN,
    metalInstancesEndpointScheme:
      process.env.APP_METAL_INSTANCES_ENDPOINT_SCHEME,
    metalInstancesEndpointPort: tryNumber(
      process.env.APP_METAL_INSTANCES_ENDPOINT_PORT,
    ),
    metalInstancesIdleThresholdSeconds: tryNumber(
      process.env.APP_METAL_INSTANCES_IDLE_THRESHOLD_SECONDS,
    ),
  });

  return {
    ...config,
    ...overrides,
  };
}

async function createDnsService(
  zone: string,
  subdomain: string,
  config: EnvVars,
  log: Logger,
): Promise<DnsService> {
  const localstackEnabled = hasFeatureFlag(
    config.enabledFeatures,
    FeatureFlag.LOCALSTACK,
  );
  return localstackEnabled
    ? await LocalStackDnsService.create(zone, subdomain, log)
    : await Route53DnsService.create(zone, subdomain, log);
}
