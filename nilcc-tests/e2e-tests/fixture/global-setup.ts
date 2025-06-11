import {
  type ChildProcess,
  type SpawnOptions,
  spawn,
} from "node:child_process";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import dockerCompose from "docker-compose";
import { Client, type ClientConfig } from "pg";

const filename = fileURLToPath(import.meta.url);
const current_dirname = dirname(filename);

let apiProcess: ChildProcess;

export const API_CONFIG = {
  APP_DB_URI: "psql://postgres:postgres@localhost:35432/postgres",
  APP_ENABLED_FEATURES:
    "openapi,prometheus-metrics,migrations,response-validation,localstack,pretty-logs",
  APP_LOG_LEVEL: "debug",
  APP_METRICS_PORT: "9091",
  APP_HTTP_API_PORT: "8081",
  APP_METAL_INSTANCE_API_KEY: "your-metal-instance-api-key",
  APP_USER_API_KEY: "your-user-api-key",
  APP_WORKLOAD_DNS_DOMAIN: "localhost",
  APP_METAL_INSTANCE_DNS_DOMAIN: "localhost",
};

export const API_URL = `http://localhost:${API_CONFIG.APP_HTTP_API_PORT}`;
export const DB_PORT = 35432;
export const LOCALSTACK_PORT = 4566;

const DOCKER_COMPOSE_OPTIONS = {
  cwd: `${current_dirname}/../../docker`,
  composeOptions: [["--project-name", "nilcc-tests"]],
};

const NILCC_AGENT_BUILD_OPTIONS: [string, string[], SpawnOptions] = [
  "cargo",
  ["build"],
  { cwd: "../nilcc-agent" },
];

async function spawnAsync(
  command: string,
  args: string[],
  options: SpawnOptions,
) {
  return new Promise((resolve, reject) => {
    const proc = spawn(command, args, { ...options, stdio: "inherit" });
    proc.on("close", (code) => {
      if (code === 0) resolve(code);
      else reject(new Error(`Process ${command} exited with code ${code}`));
    });
    proc.on("error", reject);
  });
}

async function waitForResource(
  check: () => Promise<void>,
  resourceName: string,
  maxRetries = 30,
  retryDelay = 1000,
): Promise<void> {
  console.log(`⏳ Waiting for ${resourceName} to be ready...`);
  for (let retry = 1; retry <= maxRetries; retry++) {
    try {
      await check();
      console.log(`✅ ${resourceName} is ready.`);
      return;
    } catch (error) {
      if (retry < maxRetries) {
        console.log(
          `Attempt ${retry}/${maxRetries}: ${resourceName} not ready (${error}). Retrying in ${retryDelay / 1000}s...`,
        );
        await new Promise((resolve) => setTimeout(resolve, retryDelay));
      }
    }
  }
  throw new Error(
    `Resource ${resourceName} did not become ready after ${maxRetries} attempts.`,
  );
}

async function waitForPostgres(): Promise<void> {
  const dbConfig: ClientConfig = {
    host: "localhost",
    port: DB_PORT,
    user: "postgres",
    password: "postgres",
    database: "postgres",
    connectionTimeoutMillis: 2000,
  };
  const check = async () => {
    const client = new Client(dbConfig);
    try {
      await client.connect();
    } finally {
      await client.end().catch(() => {});
    }
  };
  await waitForResource(check, "PostgreSQL");
}

async function waitForLocalstack(): Promise<void> {
  const check = async () => {
    const response = await fetch(
      `http://localhost:${LOCALSTACK_PORT}/_localstack/health`,
    );
    if (!response.ok) throw new Error(`Status ${response.status}`);
  };
  await waitForResource(check, "LocalStack");
}

async function waitForApi(): Promise<void> {
  const check = async () => {
    const response = await fetch(`${API_URL}/health`);
    if (!response.ok) throw new Error(`Status ${response.status}`);
  };
  await waitForResource(check, "API");
}

async function startDockerContainers() {
  console.log("🚀 Starting docker containers...");
  try {
    await dockerCompose.upAll({
      ...DOCKER_COMPOSE_OPTIONS,
      commandOptions: ["--force-recreate", "--renew-anon-volumes"],
    });

    await waitForPostgres();
    await waitForLocalstack();
    console.log("✅ Containers and services are ready.");
  } catch (error) {
    console.error("❌ Error starting containers: ", error);
    await process.exit(1);
  }
}

async function startApiServer() {
  const apiCwd = "../nilcc-api";
  console.log(`🚀 Installing API dependencies in ${apiCwd}...`);
  await spawnAsync("pnpm", ["install"], { cwd: apiCwd });
  console.log("✅ API dependencies installed.");

  console.log("🚀 Starting API server...");
  apiProcess = spawn("pnpm", ["start"], {
    cwd: apiCwd,
    env: { ...process.env, ...API_CONFIG },
    detached: true,
  });

  apiProcess.stdout?.on("data", (data) => console.log(`[API] ${data}`));
  apiProcess.stderr?.on("data", (data) => console.error(`[API ERROR] ${data}`));

  await waitForApi();
}

export async function setup() {
  console.log("⚙️  Building rust agent...");
  await spawnAsync(...NILCC_AGENT_BUILD_OPTIONS);
  console.log("✅ Rust agent built successfully.");

  await startDockerContainers();
  await startApiServer();

  console.log("✅ Setup complete. Ready for tests.");
}

export async function teardown() {
  if (process.env.SKIP_TEARDOWN) {
    console.log("🟡 Skipping teardown due to SKIP_TEARDOWN flag.");
    return;
  }

  if (apiProcess?.pid) {
    console.log("🛑 Stopping API server...");
    try {
      // Use process group killing to ensure child processes are also terminated
      process.kill(-apiProcess.pid, "SIGTERM");
      console.log("✅ API server stopped.");
    } catch (error) {
      console.error("❌ Error stopping API server: ", error);
    }
  }

  console.log("🛑 Removing containers...");
  try {
    await dockerCompose.downAll({
      ...DOCKER_COMPOSE_OPTIONS,
      commandOptions: ["--volumes"], // Also remove volumes to ensure a full clean up
    });
    console.log("✅ Containers removed successfully.");
  } catch (error) {
    console.error("❌ Error removing containers: ", error);
    process.exit(1);
  }
}
