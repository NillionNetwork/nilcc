/**
 * Mock nilcc-agent server for local development and testing.
 *
 * Implements the nilcc-agent HTTP API so you can test workload creation
 * via nilcc-api without a real AMD SEV-SNP metal instance.
 *
 * Also serves as a mock artifact metadata server and seeds the nilcc-api
 * with a default workload tier and artifact version on startup.
 *
 * Environment variables:
 *   NILCC_API_URL          - URL of nilcc-api (e.g. http://nilcc-api:8080)
 *   METAL_INSTANCE_ID      - UUID for this mock agent
 *   METAL_INSTANCE_API_KEY - x-api-key to authenticate with nilcc-api
 *   ADMIN_API_KEY          - x-api-key for nilcc-api admin endpoints
 *   AGENT_TOKEN            - Bearer token nilcc-api uses to auth with this agent
 *   PUBLIC_IP              - IP this agent advertises to nilcc-api (auto-detected if unset)
 *   HOSTNAME_              - Hostname to register as (defaults to os.hostname())
 *   PORT                   - Port to listen on (default: 35433)
 *   MEMORY_MB              - Total memory in MB (default: 262144)
 *   CPUS                   - Total CPUs (default: 128)
 *   DISK_GB                - Total disk in GB (default: 2048)
 *   GPUS                   - Total GPUs (default: 4)
 *   AGENT_VERSION          - Reported agent version (default: "mock-1.0.0")
 *   ARTIFACTS_VERSION      - Comma-separated artifact versions (default: "1.0.0")
 *   HEARTBEAT_INTERVAL_MS  - Heartbeat interval in ms (default: 30000)
 *   RECONCILE_INTERVAL_MS  - Retry interval while API is unavailable (default: 5000)
 *   SEED_TIER_NAME         - Name of seeded tier (default: "mock-standard")
 *   SEED_TIER_CPUS         - CPUs in seeded tier (default: CPUS - 1, min 1)
 *   SEED_TIER_MEMORY_MB    - Memory in seeded tier (default: MEMORY_MB - 256, min 256)
 *   SEED_TIER_DISK_GB      - Disk in seeded tier (default: DISK_GB - 5, min 5)
 *   SEED_TIER_GPUS         - GPUs in seeded tier (default: GPUS)
 */

import { randomUUID } from "node:crypto";
import os from "node:os";
import { serve } from "@hono/node-server";
import { Hono } from "hono";
import { bearerAuth } from "hono/bearer-auth";

// ---------------------------------------------------------------------------
// Config
// ---------------------------------------------------------------------------

function readIntEnv(name: string, fallback: number): number {
  const raw = process.env[name];
  if (raw === undefined) return fallback;
  const parsed = Number.parseInt(raw, 10);
  return Number.isFinite(parsed) ? parsed : fallback;
}

const PORT = readIntEnv("PORT", 35433);
const NILCC_API_URL = process.env.NILCC_API_URL ?? "http://nilcc-api:8080";
const METAL_INSTANCE_ID = process.env.METAL_INSTANCE_ID ?? randomUUID();
const METAL_INSTANCE_API_KEY =
  process.env.METAL_INSTANCE_API_KEY ?? "your-metal-instance-api-key";
const ADMIN_API_KEY = process.env.ADMIN_API_KEY ?? "admin-api-key";
const AGENT_TOKEN = process.env.AGENT_TOKEN ?? "mock-agent-token";
const HOSTNAME_ = process.env.HOSTNAME_ ?? os.hostname();
const MIN_MEMORY_MB = 131072;
const MIN_CPUS = 64;
const MIN_DISK_GB = 1024;
const MIN_GPUS = 4;
const MEMORY_MB = Math.max(MIN_MEMORY_MB, readIntEnv("MEMORY_MB", 262144));
const CPUS = Math.max(MIN_CPUS, readIntEnv("CPUS", 128));
const DISK_GB = Math.max(MIN_DISK_GB, readIntEnv("DISK_GB", 2048));
const GPUS = Math.max(MIN_GPUS, readIntEnv("GPUS", 4));
const AGENT_VERSION = process.env.AGENT_VERSION ?? "mock-1.0.0";
const ARTIFACTS_VERSIONS = (process.env.ARTIFACTS_VERSION ?? "1.0.0")
  .split(",")
  .map((v) => v.trim());
const HEARTBEAT_INTERVAL_MS = readIntEnv("HEARTBEAT_INTERVAL_MS", 30000);
const RECONCILE_INTERVAL_MS = readIntEnv("RECONCILE_INTERVAL_MS", 5000);

const SEED_TIER_NAME = process.env.SEED_TIER_NAME ?? "mock-standard";
const SEED_TIER_CPUS = Math.max(1, readIntEnv("SEED_TIER_CPUS", 1));
const SEED_TIER_MEMORY_MB = Math.max(
  256,
  readIntEnv("SEED_TIER_MEMORY_MB", 8192),
);
const SEED_TIER_DISK_GB = Math.max(5, readIntEnv("SEED_TIER_DISK_GB", 5));
const SEED_TIER_GPUS = Math.max(0, readIntEnv("SEED_TIER_GPUS", 0));

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface Workload {
  id: string;
  domain: string;
  enabled: boolean;
  createRequest: Record<string, unknown>;
}

interface AgentError {
  errorCode: string;
  message: string;
}

// ---------------------------------------------------------------------------
// In-memory workload store
// ---------------------------------------------------------------------------

const workloads = new Map<string, Workload>();

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function getLocalIp(): string {
  if (process.env.PUBLIC_IP) return process.env.PUBLIC_IP;
  for (const iface of Object.values(os.networkInterfaces())) {
    for (const addr of iface ?? []) {
      if (addr.family === "IPv4" && !addr.internal) return addr.address;
    }
  }
  return "127.0.0.1";
}

function log(msg: string): void {
  console.log(`[mock-agent ${METAL_INSTANCE_ID.slice(0, 8)}] ${msg}`);
}

function agentError(errorCode: string, message: string): AgentError {
  return { errorCode, message };
}

function requireWorkload(id: string): Workload | AgentError {
  const w = workloads.get(id);
  if (!w) return agentError("WORKLOAD_NOT_FOUND", `Workload ${id} not found`);
  return w;
}

function isWorkload(v: Workload | AgentError): v is Workload {
  return !("errorCode" in v);
}

function adminPost(path: string, body: unknown): Promise<Response> {
  return fetch(`${NILCC_API_URL}${path}`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "x-api-key": ADMIN_API_KEY,
    },
    body: JSON.stringify(body),
  });
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

const app = new Hono();

// Health — no auth
app.get("/health", (c) => c.text("OK"));

// Artifact metadata — no auth, consumed by nilcc-api when enabling a version.
// Matches: GET /{version}/metadata.json
app.get("/:version/metadata.json", (c) =>
  c.json({ built_at: Math.floor(Date.now() / 1000) }),
);

// All agent routes require Bearer token
const authenticated = new Hono().use(bearerAuth({ token: AGENT_TOKEN }));

// ── System ──────────────────────────────────────────────────────────────────

authenticated.get("/api/v1/system/agent/version", (c) =>
  c.json({ version: AGENT_VERSION }),
);

authenticated.get("/api/v1/system/artifacts/versions", (c) =>
  c.json({ versions: ARTIFACTS_VERSIONS }),
);

authenticated.get("/api/v1/system/artifacts/changelog", (c) =>
  c.json({ entries: [] }),
);

authenticated.post("/api/v1/system/artifacts/install", (c) => c.json({}));

authenticated.post("/api/v1/system/artifacts/cleanup", (c) =>
  c.json({ versionsDeleted: [] }),
);

authenticated.get("/api/v1/system/verifier/keys", (c) => c.json([]));

authenticated.post("/api/v1/system/agent/upgrade", (c) => c.json({}));

// ── Workloads ────────────────────────────────────────────────────────────────

authenticated.get("/api/v1/workloads/list", (c) =>
  c.json(
    Array.from(workloads.values()).map(({ id, enabled, domain }) => ({
      id,
      enabled,
      domain,
    })),
  ),
);

authenticated.post("/api/v1/workloads/create", async (c) => {
  const body = await c.req.json<Record<string, unknown>>();
  const id = typeof body.id === "string" ? body.id : undefined;
  const domain = typeof body.domain === "string" ? body.domain : "unknown";

  if (!id) {
    return c.json(agentError("MALFORMED_REQUEST", "Missing workload id"), 400);
  }
  const existing = workloads.get(id);
  if (!existing) {
    workloads.set(id, { id, domain, enabled: true, createRequest: body });
    log(`Created workload ${id} (domain: ${domain})`);
  } else {
    existing.domain = domain;
    existing.enabled = true;
    existing.createRequest = body;
    log(`Updated workload ${id} (domain: ${domain})`);
  }
  return c.json({ id });
});

authenticated.post("/api/v1/workloads/delete", async (c) => {
  const { id } = await c.req.json<{ id: string }>();
  if (!workloads.has(id)) {
    return c.json(
      agentError("WORKLOAD_NOT_FOUND", `Workload ${id} not found`),
      404,
    );
  }
  workloads.delete(id);
  log(`Deleted workload ${id}`);
  return c.json({});
});

authenticated.post("/api/v1/workloads/start", async (c) => {
  const { id } = await c.req.json<{ id: string }>();
  const result = requireWorkload(id);
  if (!isWorkload(result)) return c.json(result, 404);
  result.enabled = true;
  return c.json({});
});

authenticated.post("/api/v1/workloads/stop", async (c) => {
  const { id } = await c.req.json<{ id: string }>();
  const result = requireWorkload(id);
  if (!isWorkload(result)) return c.json(result, 404);
  result.enabled = false;
  return c.json({});
});

authenticated.post("/api/v1/workloads/restart", async (c) => {
  const { id } = await c.req.json<{ id: string }>();
  const result = requireWorkload(id);
  if (!isWorkload(result)) return c.json(result, 404);
  log(`Restarted workload ${id}`);
  return c.json({});
});

// ── Per-workload ─────────────────────────────────────────────────────────────

authenticated.get("/api/v1/workloads/:workloadId/health", (c) => {
  const result = requireWorkload(c.req.param("workloadId"));
  if (!isWorkload(result)) return c.json(result, 404);
  return c.json({ status: "healthy" });
});

authenticated.get("/api/v1/workloads/:workloadId/containers/list", (c) => {
  const result = requireWorkload(c.req.param("workloadId"));
  if (!isWorkload(result)) return c.json(result, 404);
  return c.json([
    {
      names: ["mock-container-1"],
      image: "ghcr.io/example/mock-service:latest",
      imageId:
        "sha256:0000000000000000000000000000000000000000000000000000000000000000",
      state: "running",
    },
  ]);
});

authenticated.get("/api/v1/workloads/:workloadId/containers/logs", (c) => {
  const result = requireWorkload(c.req.param("workloadId"));
  if (!isWorkload(result)) return c.json(result, 404);
  const maxLines = Math.min(
    Number.parseInt(c.req.query("maxLines") ?? "10", 10),
    1000,
  );
  const lines = Array.from(
    { length: Math.min(maxLines, 5) },
    (_, i) => `[mock] container log line ${i + 1}`,
  );
  return c.json({ lines });
});

authenticated.get("/api/v1/workloads/:workloadId/system/logs", (c) => {
  const result = requireWorkload(c.req.param("workloadId"));
  if (!isWorkload(result)) return c.json(result, 404);
  const maxLines = Math.min(
    Number.parseInt(c.req.query("maxLines") ?? "10", 10),
    1000,
  );
  const lines = Array.from(
    { length: Math.min(maxLines, 5) },
    (_, i) => `[mock] system log line ${i + 1}`,
  );
  return c.json({ lines });
});

authenticated.get("/api/v1/workloads/:workloadId/system/stats", (c) => {
  const result = requireWorkload(c.req.param("workloadId"));
  if (!isWorkload(result)) return c.json(result, 404);
  return c.json({
    memory: {
      total: MEMORY_MB * 1024 * 1024,
      used: Math.floor(MEMORY_MB * 1024 * 1024 * 0.3),
    },
    cpus: Array.from({ length: CPUS }, (_, i) => ({
      name: `CPU ${i}`,
      usage: Math.random() * 20,
      frequency: 3200,
    })),
    disks: [
      {
        name: "sda",
        mountPoint: "/",
        filesystem: "ext4",
        size: DISK_GB * 1024 * 1024 * 1024,
        used: Math.floor(DISK_GB * 1024 * 1024 * 1024 * 0.1),
      },
    ],
  });
});

app.route("/", authenticated);

// ---------------------------------------------------------------------------
// Registration, heartbeat, and seeding
// ---------------------------------------------------------------------------

async function registerWithApi(publicIp: string): Promise<boolean> {
  try {
    const res = await fetch(
      `${NILCC_API_URL}/api/v1/metal-instances/register`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-api-key": METAL_INSTANCE_API_KEY,
        },
        body: JSON.stringify({
          metalInstanceId: METAL_INSTANCE_ID,
          agentVersion: AGENT_VERSION,
          publicIp,
          token: AGENT_TOKEN,
          hostname: HOSTNAME_,
          memoryMb: { reserved: 0, total: MEMORY_MB },
          cpus: { reserved: 0, total: CPUS },
          diskSpaceGb: { reserved: 0, total: DISK_GB },
          gpus: GPUS,
        }),
      },
    );
    if (res.ok) {
      log(`Registered with nilcc-api at ${NILCC_API_URL}`);
      return true;
    }
    log(`Registration failed (${res.status}): ${await res.text()}`);
    return false;
  } catch (err) {
    log(`Registration error: ${err}`);
    return false;
  }
}

async function sendHeartbeat(): Promise<{
  ok: boolean;
  shouldReregister: boolean;
}> {
  try {
    const res = await fetch(
      `${NILCC_API_URL}/api/v1/metal-instances/heartbeat`,
      {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
          "x-api-key": METAL_INSTANCE_API_KEY,
        },
        body: JSON.stringify({
          metalInstanceId: METAL_INSTANCE_ID,
          availableArtifactVersions: ARTIFACTS_VERSIONS,
        }),
      },
    );
    if (res.ok) {
      log("Heartbeat sent");
      return { ok: true, shouldReregister: false };
    }
    const body = await res.text();
    log(`Heartbeat failed (${res.status}): ${body}`);
    return {
      ok: false,
      shouldReregister: res.status === 404,
    };
  } catch (err) {
    log(`Heartbeat error: ${err}`);
    return { ok: false, shouldReregister: false };
  }
}

/**
 * Seed nilcc-api with a default workload tier and the configured artifact
 * versions. Idempotent — existing entries are left untouched.
 */
async function seedApi(): Promise<void> {
  // Workload tier
  try {
    const res = await adminPost("/api/v1/workload-tiers/create", {
      name: SEED_TIER_NAME,
      cpus: SEED_TIER_CPUS,
      gpus: SEED_TIER_GPUS,
      memoryMb: SEED_TIER_MEMORY_MB,
      diskGb: SEED_TIER_DISK_GB,
      cost: 1,
    });
    if (res.ok) {
      log(
        `Seeded workload tier "${SEED_TIER_NAME}" (${SEED_TIER_CPUS} cpu, ${SEED_TIER_MEMORY_MB} MB, ${SEED_TIER_DISK_GB} GB, ${SEED_TIER_GPUS} gpu)`,
      );
    } else {
      const body = await res.text();
      // Conflict means it already exists — that's fine
      if (res.status !== 409) {
        log(`Workload tier seed failed (${res.status}): ${body}`);
      }
    }
  } catch (err) {
    log(`Workload tier seed error: ${err}`);
  }

  // Artifact versions
  for (const version of ARTIFACTS_VERSIONS) {
    try {
      const res = await adminPost("/api/v1/artifacts/enable", { version });
      if (res.ok) {
        log(`Enabled artifact version "${version}"`);
      } else {
        const body = await res.text();
        if (res.status !== 409) {
          log(
            `Artifact enable failed for "${version}" (${res.status}): ${body}`,
          );
        }
      }
    } catch (err) {
      log(`Artifact enable error for "${version}": ${err}`);
    }
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function reconcileWithApi(publicIp: string): Promise<void> {
  let isRegistered = false;
  let isSeeded = false;
  while (true) {
    if (!isRegistered) {
      isRegistered = await registerWithApi(publicIp);
      if (isRegistered) {
        isSeeded = false;
      } else {
        await sleep(RECONCILE_INTERVAL_MS);
        continue;
      }
    }

    if (!isSeeded) {
      await seedApi();
      isSeeded = true;
    }

    const heartbeat = await sendHeartbeat();
    if (!heartbeat.ok) {
      if (heartbeat.shouldReregister) {
        log("Metal instance not found by nilcc-api, will re-register");
      }
      isRegistered = false;
      await sleep(RECONCILE_INTERVAL_MS);
      continue;
    }

    await sleep(HEARTBEAT_INTERVAL_MS);
  }
}

// ---------------------------------------------------------------------------
// Start
// ---------------------------------------------------------------------------

const publicIp = getLocalIp();

serve({ fetch: app.fetch, port: PORT }, () => {
  log(`Listening on port ${PORT} (publicIp: ${publicIp})`);
  log(`Metal instance ID: ${METAL_INSTANCE_ID}`);
  log(`Artifact versions: ${ARTIFACTS_VERSIONS.join(", ")}`);
  log(
    `Seed tier: ${SEED_TIER_NAME} (${SEED_TIER_CPUS} cpu, ${SEED_TIER_MEMORY_MB} MB, ${SEED_TIER_DISK_GB} GB, ${SEED_TIER_GPUS} gpu)`,
  );

  setTimeout(() => {
    void reconcileWithApi(publicIp);
  }, 3000);
});
