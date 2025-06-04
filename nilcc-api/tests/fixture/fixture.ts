import { faker } from "@faker-js/faker";
import dotenv from "dotenv";
import type { Hono } from "hono";
import { type Logger, pino } from "pino";
import { DataSource } from "typeorm";
import { buildApp } from "#/app";
import { type AppBindings, type AppEnv, loadBindings } from "#/env";
import { MetalInstanceClient, WorkloadClient } from "./test-client";

export type TestFixture = {
  app: Hono<AppEnv>;
  log: Logger;
  bindings: AppBindings;
  workloadClient: WorkloadClient;
  metalInstanceClient: MetalInstanceClient;
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

  const baseDBUri = process.env.APP_DB_URI;
  const thisDescribeDBUri = `${baseDBUri}-${id}`;
  await createDatabase(thisDescribeDBUri, log);

  const bindings = (await loadBindings({
    dbUri: thisDescribeDBUri,
  })) as AppBindings;

  log.info("Creating App");
  const { app } = await buildApp(bindings);

  const workloadClient = new WorkloadClient({
    app,
    bindings,
  });
  const metalInstanceClient = new MetalInstanceClient({
    app,
    bindings,
  });
  log.info("Test suite ready");
  return { app, log, bindings, workloadClient, metalInstanceClient };
}

async function createDatabase(dbUri: string, log: Logger): Promise<void> {
  const segments: string[] = dbUri.split("/");
  const dbName: string | undefined = segments.pop();
  segments.push("postgres");
  const systemDbUri: string = segments.join("/");

  const systemDataSource = new DataSource({
    type: "postgres",
    url: systemDbUri,
  });

  try {
    await systemDataSource.initialize();

    await systemDataSource.query(`CREATE DATABASE "${dbName}"`);
    log.info(`Database "${dbName}" created successfully.`);
  } catch (error) {
    log.error("Error creating database:", error);
  } finally {
    if (systemDataSource.isInitialized) {
      await systemDataSource.destroy();
    }
  }
}
