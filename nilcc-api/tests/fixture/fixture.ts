import dotenv from "dotenv";
import type { Hono } from "hono";
import { buildApp } from "#/app";
import { type AppEnv, loadBindings, AppBindings } from "#/env";
import { WorkloadClient } from "./test-client";

export type TestFixture = {
  app: Hono<AppEnv>;
  bindings: AppBindings;
  workload: WorkloadClient;
};

export async function buildFixture(): Promise<TestFixture> {
  dotenv.config({ path: [".env.test"] });

  const bindings = (await loadBindings()) as AppBindings;
  const { app } = await buildApp(bindings);

  const workload = new WorkloadClient({
    app,
  });

  return { app, bindings, workload };
}
