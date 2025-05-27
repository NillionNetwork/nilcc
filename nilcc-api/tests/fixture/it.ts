import * as vitest from "vitest";
import type { App } from "#/app";
import type { AppBindings } from "#/env";
import { buildFixture, type TestFixture } from "./fixture";
import type { WorkloadClient } from "./test-client";

export type FixtureContext = {
  app: App;
  bindings: AppBindings;
  workload: WorkloadClient;
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
    workload: async ({}, use) => {
      if (!fixture) throw new Error("Fixture is not initialized");
      await use(fixture.workload);
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
      const { bindings } = fixture;
      const { dataSource } = bindings;

      await dataSource.destroy();

      await fn(fixture);
    });

  return { beforeAll, afterAll, it };
}
