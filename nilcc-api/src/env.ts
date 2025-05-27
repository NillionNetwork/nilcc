import { z } from "zod";
import type { Logger } from "pino";
import { createLogger } from "#/common/logger";
import { buildDataSource } from "#/data-source";
import { DataSource } from "typeorm";
export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;

export const FeatureFlag = {
  OPENAPI_DOCS: "openapi-docs",
  PROMETHEUS_METRICS: "prometheus-metrics",
  MIGRATIONS: "migrations",
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

export type AppVariables = {};

export type AppBindings = {
  config: EnvVars;
  dataSource: DataSource;
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
    }
  }
}

export async function loadBindings(
  overrides: Partial<EnvVars> = {},
): Promise<AppBindings> {
  const config = parseConfigFromEnv(overrides);

  const dataSource = await buildDataSource(config);

  return {
    config,
    dataSource,
    log: createLogger(config.logLevel),
  };
}

export function parseConfigFromEnv(overrides: Partial<EnvVars>): EnvVars {
  const config = EnvVarsSchema.parse({
    dbUri: process.env.APP_DB_URI,
    enabledFeatures: process.env.APP_ENABLED_FEATURES,
    logLevel: process.env.APP_LOG_LEVEL,
    metricsPort: Number(process.env.APP_METRICS_PORT),
    httpApiPort: Number(process.env.APP_HTTP_API_PORT),
  });

  return {
    ...config,
    ...overrides,
  };
}
