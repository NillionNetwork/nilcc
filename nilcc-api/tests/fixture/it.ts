import type { Logger } from "pino";
import * as vitest from "vitest";
import type { App } from "#/app";
import type { AppBindings } from "#/env";
import { buildFixture, type TestFixture } from "./fixture";
import type { MetalInstanceClient, WorkloadClient } from "./test-client";

export type FixtureContext = {
  app: App;
  bindings: AppBindings;
  workloadClient: WorkloadClient;
  metalInstanceClient: MetalInstanceClient; // Assuming you have a metal instance client similar to workload client
};

type TestFixtureExtension = {
  it: vitest.TestAPI<FixtureContext>;
  beforeAll: (fn: (ctx: FixtureContext) => Promise<void>) => void;
  afterAll: (fn: (ctx: FixtureContext) => Promise<void>) => void;
};

export function createTestFixtureExtension(): TestFixtureExtension {
  let fixture: TestFixture | null = null;

  // biome-ignore-start lint/correctness/noEmptyPattern: Vitest fixture API requires this parameter structure
  const it = vitest.test.extend<FixtureContext>({
    app: async ({}, use) => {
      if (!fixture) throw new Error("Fixture is not initialized");
      await use(fixture.app);
    },
    bindings: async ({}, use) => {
      if (!fixture) throw new Error("Fixture is not initialized");
      await use(fixture.bindings);
    },
    workloadClient: async ({}, use) => {
      if (!fixture) throw new Error("Fixture is not initialized");
      await use(fixture.workloadClient);
    },
    metalInstanceClient: async ({}, use) => {
      if (!fixture) throw new Error("Fixture is not initialized");
      await use(fixture.metalInstanceClient);
    },
  });
  // biome-ignore-end lint/correctness/noEmptyPattern: Vitest fixture API requires this parameter structure

  const beforeAll = (fn: (ctx: FixtureContext) => Promise<void>) =>
    vitest.beforeAll(async () => {
      try {
        fixture = await buildFixture();
        await fn(fixture);
      } catch (error) {
        console.error("Fixture setup failed:", error);
        throw error;
      }
    });

  const afterAll = (fn: (ctx: FixtureContext) => Promise<void>) =>
    vitest.afterAll(async () => {
      if (!fixture) throw new Error("Fixture is not initialized");
      const { bindings, log } = fixture;
      const { dataSource } = bindings;

      log.debug("Dropping database and destroying data source");
      await dataSource.dropDatabase();
      await dataSource.destroy();
      await flushLogger(log);
      await flushLogger(bindings.log);
      await fn(fixture);
    });

  return { beforeAll, afterAll, it };
}

async function flushLogger(logger: Logger) {
  return new Promise<void>((resolve) => {
    logger.flush(() => {
      resolve();
    });
  });
}
