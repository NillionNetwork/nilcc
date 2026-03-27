import { connect } from "node:net";
import { dirname } from "node:path";
import { setTimeout as sleep } from "node:timers/promises";
import { fileURLToPath } from "node:url";
import dockerCompose from "docker-compose";
import type { TestProject } from "vitest/node";

const filename = fileURLToPath(import.meta.url);
const current_dirname = dirname(filename);

const MAX_RETRIES = 300;
const composeOptions = {
  cwd: `${current_dirname}/../docker`,
  composeOptions: [["--project-name", "nilcc-tests"]],
};

export async function setup(_project: TestProject) {
  console.log("Starting containers...");
  try {
    // Check if containers are already running
    const psResult = await dockerCompose.ps(composeOptions);
    const allServicesUp =
      psResult.data.services?.length > 0 &&
      psResult.data.services.every((service) => service.state?.includes("Up"));

    if (allServicesUp) {
      console.log("Containers already running, skipping startup.");
      return;
    }

    await dockerCompose.upAll(composeOptions);
    let retry = 0;
    for (; retry < MAX_RETRIES; retry++) {
      const result = await dockerCompose.ps(composeOptions);
      if (
        result.data.services.every((service) => service.state.includes("Up")) &&
        (await allEndpointsReady())
      ) {
        break;
      }
      await sleep(200);
    }
    if (retry >= MAX_RETRIES) {
      console.error("Error starting containers: timeout");
      process.exit(1);
    }
    // We need sleep 1 sec to be sure that the AboutResponse.started is at least 1 sec earlier than the tests start.
    await sleep(2000);
    console.log("Containers started successfully.");
  } catch (error) {
    console.error("Error starting containers: ", error);
    process.exit(1);
  }
}

export async function teardown(_project: TestProject) {
  // Skip teardown if KEEP_INFRA environment variable is set
  if (process.env.KEEP_INFRA === "true") {
    console.log("Keeping infrastructure running as KEEP_INFRA=true");
    return;
  }

  console.log("Removing containers...");
  try {
    await dockerCompose.downAll(composeOptions);
    console.log("Containers removed successfully.");
  } catch (error) {
    console.error("Error removing containers: ", error);
    process.exit(1);
  }
}

async function allEndpointsReady(): Promise<boolean> {
  const checks = [
    waitForTcpPort("127.0.0.1", 35432),
    waitForHttp("http://127.0.0.1:14566/_localstack/health"),
    waitForJsonRpc("http://127.0.0.1:38545"),
    waitForHttp("http://127.0.0.1:35435"),
    waitForHttp("http://127.0.0.1:35436"),
  ];

  const results = await Promise.all(checks);
  return results.every(Boolean);
}

async function waitForHttp(url: string): Promise<boolean> {
  try {
    const response = await fetch(url, {
      signal: AbortSignal.timeout(1000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

async function waitForJsonRpc(url: string): Promise<boolean> {
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        jsonrpc: "2.0",
        method: "eth_chainId",
        params: [],
        id: 1,
      }),
      signal: AbortSignal.timeout(1000),
    });
    return response.ok;
  } catch {
    return false;
  }
}

async function waitForTcpPort(host: string, port: number): Promise<boolean> {
  return new Promise((resolve) => {
    const socket = connect({ host, port });

    socket.setTimeout(1000);

    socket.once("connect", () => {
      socket.end();
      resolve(true);
    });

    socket.once("timeout", () => {
      socket.destroy();
      resolve(false);
    });

    socket.once("error", () => {
      socket.destroy();
      resolve(false);
    });
  });
}
