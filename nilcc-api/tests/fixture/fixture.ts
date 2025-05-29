import { faker } from "@faker-js/faker";
import dotenv from "dotenv";
import type { Hono } from "hono";
import { type Logger, pino } from "pino";
import { buildApp } from "#/app";
import { type AppBindings, type AppEnv, loadBindings } from "#/env";
import { WorkloadClient } from "./test-client";

export type TestFixture = {
  app: Hono<AppEnv>;
  log: Logger;
  bindings: AppBindings;
  workload: WorkloadClient;
};

function createTestLogger(id: string): Logger {
  return pino({
    transport: {
      target: "pino-pretty",
      options: {
        sync: true,
        singleLine: true,
        messageFormat: `${id} - {msg}`,
      },
    },
  });
}

export async function buildFixture(): Promise<TestFixture> {
  dotenv.config({ path: [".env.test"] });
  const id = faker.string.alphanumeric({ length: 4, casing: "lower" });
  const log = createTestLogger(id);

  log.info("Creating Binding");
  const bindings = (await loadBindings()) as AppBindings;

  log.info("Creating App");
  const { app } = await buildApp(bindings);

  const workload = new WorkloadClient({
    app,
  });
  log.info("Test suite ready");
  return { app, log, bindings, workload };
}
