import type { Logger } from "pino";
import type { DataSource, QueryRunner } from "typeorm";
import { z } from "zod";
import { createLogger } from "#/common/logger";
import { buildDataSource } from "#/data-source";
import { MetalInstanceService } from "#/metal-instance/metal-instance.service";
import { WorkloadService } from "#/workload/workload.service";
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

export type AppServices = {
  metalInstance: MetalInstanceService;
  workload: WorkloadService;
  dns: DnsService;
};

export type AppBindings = {
  config: EnvVars;
  dataSource: DataSource;
  services: AppServices;
  log: Logger;
};

export const EnvVarsSchema = z.object({
  dbUri: z.string().startsWith("psql://"),
  enabledFeatures: z
    .string()
    .transform((d) => d.split(",").map((e) => e.trim())),
  logLevel: z.enum(LOG_LEVELS),
  metricsPort: z.number().int().positive(),
  httpApiPort: z.number().int().positive(),
  metalInstanceApiKey: z.string(),
  metalInstanceDnsDomain: z.string(),
  workloadDnsDomain: z.string(),
});

export type EnvVars = z.infer<typeof EnvVarsSchema>;

// Use interface merging to define expected app vars
declare global {
  namespace NodeJS {
    interface ProcessEnv {
      APP_DB_URI: string;
      APP_ENABLED_FEATURES: string;
      APP_LOG_LEVEL: string;
      APP_METRICS_PORT?: number;
      APP_HTTP_API_PORT: number;
      APP_METAL_INSTANCE_API_KEY: string;
      APP_WORKLOAD_DNS_DOMAIN: string;
      APP_METAL_INSTANCE_DNS_DOMAIN: string;
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
  const dnsService = hasFeatureFlag(
    config.enabledFeatures,
    FeatureFlag.LOCALSTACK,
  )
    ? new LocalStackDnsService(config.workloadDnsDomain)
    : new Route53DnsService();

  log.debug("Using DNS service: %s", dnsService.constructor.name);

  const metalInstanceService = new MetalInstanceService();
  const workloadService = new WorkloadService();

  return {
    metalInstance: metalInstanceService,
    workload: workloadService,
    dns: dnsService,
  };
}

export function parseConfigFromEnv(overrides: Partial<EnvVars>): EnvVars {
  const config = EnvVarsSchema.parse({
    dbUri: process.env.APP_DB_URI,
    enabledFeatures: process.env.APP_ENABLED_FEATURES,
    logLevel: process.env.APP_LOG_LEVEL,
    metricsPort: Number(process.env.APP_METRICS_PORT),
    httpApiPort: Number(process.env.APP_HTTP_API_PORT),
    metalInstanceApiKey: process.env.APP_METAL_INSTANCE_API_KEY,
    workloadDnsDomain: process.env.APP_WORKLOAD_DNS_DOMAIN,
    metalInstanceDnsDomain: process.env.APP_METAL_INSTANCE_DNS_DOMAIN,
  });

  return {
    ...config,
    ...overrides,
  };
}
