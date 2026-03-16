import type { Logger } from "pino";
import { Counter, Registry } from "prom-client";
import type { DataSource, QueryRunner } from "typeorm";
import { z } from "zod";
import { ApiKeyService } from "#/api-key/api-key.service";
import { AuthService } from "#/auth/auth.service";
import type { AuthContext } from "#/common/auth";
import { createLogger } from "#/common/logger";
import { buildDataSource } from "#/data-source";
import { MetalInstanceService } from "#/metal-instance/metal-instance.service";
import { NilPriceService } from "#/payment/nil-price.service";
import { PaymentService } from "#/payment/payment.service";
import { WorkloadService } from "#/workload/workload.service";
import type { AccountEntity } from "./account/account.entity";
import { AccountService } from "./account/account.service";
import { ArtifactService } from "./artifact/artifact.service";
import {
  type ArtifactClient,
  DefaultArtifactClient,
} from "./clients/artifact.client";
import {
  DefaultNilccAgentClient,
  type NilccAgentClient,
} from "./clients/nilcc-agent.client";
import {
  type DnsService,
  LocalStackDnsService,
  Route53DnsService,
} from "./dns/dns.service";
import { WorkloadTierService } from "./workload-tier/workload-tier.service";
export const LOG_LEVELS = ["debug", "info", "warn", "error"] as const;

export const FeatureFlag = {
  OPENAPI_SPEC: "openapi",
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
  account: AccountEntity;
  auth: AuthContext;
};

export type DnsServices = {
  metalInstances: DnsService;
  workloads: DnsService;
};

export type AppServices = {
  apiKey: ApiKeyService;
  metalInstance: MetalInstanceService;
  workload: WorkloadService;
  workloadTier: WorkloadTierService;
  account: AccountService;
  auth: AuthService;
  payment: PaymentService;
  artifact: ArtifactService;
  dns: DnsServices;
  time: TimeService;
  nilPrice: NilPriceService;
  nilccAgentClient: NilccAgentClient;
  artifactsClient: ArtifactClient;
};

export interface TimeService {
  getTime(): Date;
}

export type AppBindings = {
  config: EnvVars;
  dataSource: DataSource;
  services: AppServices;
  log: Logger;
  metricsRegistry: Registry;
  metrics: Metrics;
};

export type Metrics = {
  deactivatedWorkloads: Counter;
  metalInstanceHeartbeats: Counter;
};

export const EnvVarsSchema = z.object({
  dbUri: z.string().startsWith("postgres://"),
  enabledFeatures: z
    .string()
    .transform((d) => d.split(",").map((e) => e.trim())),
  logLevel: z.enum(LOG_LEVELS),
  metricsPort: z.number().int().positive(),
  httpApiPort: z.number().int().positive(),
  adminApiKey: z.string(),
  metalInstanceApiKey: z.string(),
  workloadsDnsDomain: z.string(),
  workloadsDnsZone: z.string(),
  metalInstancesDnsDomain: z.string(),
  metalInstancesDnsZone: z.string(),
  metalInstancesEndpointScheme: z.enum(["http", "https"]).default("https"),
  metalInstancesEndpointPort: z.number().default(443),
  metalInstancesIdleThresholdSeconds: z.number().default(120),
  artifactsBaseUrl: z.string(),
  requireArtifactsSemver: z.boolean(),
  jwtSecret: z.string().min(32),
  jwtExpirationSeconds: z.number().int().positive().default(86400),
  rpcUrl: z.string().url().optional(),
  burnContractAddress: z
    .string()
    .regex(/^0x[a-fA-F0-9]{40}$/)
    .optional(),
  chainId: z.number().int().positive().optional(),
  paymentStartBlock: z.number().int().nonnegative().default(0),
  coingeckoApiKey: z.string(),
  paymentPollerIntervalMs: z.number().int().positive().default(60_000),
  paymentPollerMaxBlockRange: z.number().int().positive().default(1000),
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
      APP_ADMIN_API_KEY: string;
      APP_METAL_INSTANCE_API_KEY: string;
      APP_WORKLOADS_DNS_ZONE: string;
      APP_WORKLOADS_DNS_DOMAIN: string;
      APP_METAL_INSTANCES_DNS_DOMAIN: string;
      APP_METAL_INSTANCES_DNS_ZONE: string;
      APP_METAL_INSTANCES_ENDPOINT_SCHEME?: string;
      APP_METAL_INSTANCES_ENDPOINT_PORT?: string;
      APP_METAL_INSTANCES_IDLE_THRESHOLD_SECONDS?: string;
      APP_ARTIFACTS_BASE_URL: string;
      APP_REQUIRE_ARTIFACTS_SEMVER?: string;
      APP_JWT_SECRET: string;
      APP_JWT_EXPIRATION_SECONDS?: string;
      APP_RPC_URL: string;
      APP_BURN_CONTRACT_ADDRESS: string;
      APP_CHAIN_ID: string;
      APP_PAYMENT_START_BLOCK?: string;
      APP_COINGECKO_API_KEY: string;
      APP_PAYMENT_POLLER_INTERVAL_MS?: string;
      APP_PAYMENT_POLLER_MAX_BLOCK_RANGE?: string;
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
  const metricsRegistry = new Registry();
  const metrics = createMetrics(metricsRegistry);

  const dataSource = await buildDataSource(config);
  log.debug("Initializing database");
  await dataSource.initialize();

  const services = await buildServices(config, log);

  return {
    config,
    dataSource,
    services,
    log,
    metricsRegistry,
    metrics,
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
  const workloadTierService = new WorkloadTierService();
  const accountService = new AccountService();
  const apiKeyService = new ApiKeyService();
  const authService = new AuthService();
  const paymentService = new PaymentService();
  const nilPriceService = new NilPriceService(config.coingeckoApiKey);
  const artifactService = new ArtifactService();
  const nilccAgentClient = new DefaultNilccAgentClient(
    config.metalInstancesEndpointScheme,
    config.metalInstancesDnsDomain,
    config.metalInstancesEndpointPort,
    log,
  );
  const artifactsClient = new DefaultArtifactClient(
    config.artifactsBaseUrl,
    log,
  );
  const timeService = new (class {
    getTime(): Date {
      return new Date();
    }
  })();

  return {
    apiKey: apiKeyService,
    metalInstance: metalInstanceService,
    workload: workloadService,
    workloadTier: workloadTierService,
    account: accountService,
    auth: authService,
    payment: paymentService,
    artifact: artifactService,
    dns,
    time: timeService,
    nilPrice: nilPriceService,
    nilccAgentClient,
    artifactsClient,
  };
}

export function parseConfigFromEnv(overrides: Partial<EnvVars>): EnvVars {
  const tryNumber = (n: string | undefined) =>
    n !== undefined ? Number(n) : undefined;
  const tryBoolean = (n: string | undefined) =>
    n !== undefined ? Boolean(n) : false;
  const config = EnvVarsSchema.parse({
    dbUri: process.env.APP_DB_URI,
    enabledFeatures: process.env.APP_ENABLED_FEATURES,
    logLevel: process.env.APP_LOG_LEVEL,
    metricsPort: Number(process.env.APP_METRICS_PORT),
    httpApiPort: Number(process.env.APP_HTTP_API_PORT),
    adminApiKey: process.env.APP_ADMIN_API_KEY,
    metalInstanceApiKey: process.env.APP_METAL_INSTANCE_API_KEY,
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
    artifactsBaseUrl: process.env.APP_ARTIFACTS_BASE_URL,
    requireArtifactsSemver: tryBoolean(
      process.env.APP_REQUIRE_ARTIFACTS_SEMVER,
    ),
    jwtSecret: process.env.APP_JWT_SECRET,
    jwtExpirationSeconds: tryNumber(process.env.APP_JWT_EXPIRATION_SECONDS),
    rpcUrl: process.env.APP_RPC_URL,
    burnContractAddress: process.env.APP_BURN_CONTRACT_ADDRESS,
    chainId: tryNumber(process.env.APP_CHAIN_ID),
    paymentStartBlock: tryNumber(process.env.APP_PAYMENT_START_BLOCK),
    coingeckoApiKey: process.env.APP_COINGECKO_API_KEY,
    paymentPollerIntervalMs: tryNumber(
      process.env.APP_PAYMENT_POLLER_INTERVAL_MS,
    ),
    paymentPollerMaxBlockRange: tryNumber(
      process.env.APP_PAYMENT_POLLER_MAX_BLOCK_RANGE,
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

function createMetrics(registry: Registry): Metrics {
  const registers = [registry];
  const metrics: Metrics = {
    deactivatedWorkloads: new Counter({
      name: "deactivated_workloads_total",
      help: "The total number of workloads deactivated because an account ran out of credits",
      registers,
    }),
    metalInstanceHeartbeats: new Counter({
      name: "metal_instance_heartbeats_total",
      help: "The total number of heartbeats per metal instance",
      labelNames: ["id"],
      registers,
    }),
  };
  return metrics;
}
