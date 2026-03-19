import type { Logger } from "pino";
import * as vitest from "vitest";
import type { App } from "#/app";
import type { AppBindings } from "#/env";
import { buildFixture, type TestClients, type TestFixture } from "./fixture";

export type FixtureContext = {
  app: App;
  bindings: AppBindings;
  clients: TestClients;
  issueJwt: (accountId: string, walletAddress: string) => Promise<string>;
};

type TestFixtureExtension = {
  it: vitest.TestAPI<FixtureContext>;
  beforeAll: (fn: (ctx: FixtureContext) => Promise<void>) => void;
  afterAll: (fn: (ctx: FixtureContext) => Promise<void>) => void;
};

export function createTestFixtureExtension(): TestFixtureExtension {
  let fixture: TestFixture | null = null;
  let setupError: unknown = null;

  const requireFixture = (): TestFixture => {
    if (fixture) {
      return fixture;
    }
    if (setupError) {
      throw setupError;
    }
    throw new Error("Fixture is not initialized");
  };

  // biome-ignore-start lint/correctness/noEmptyPattern: Vitest fixture API requires this parameter structure
  const it = vitest.test.extend<FixtureContext>({
    app: async ({}, use) => {
      await use(requireFixture().app);
    },
    bindings: async ({}, use) => {
      await use(requireFixture().bindings);
    },
    clients: async ({}, use) => {
      await use(requireFixture().clients);
    },
    issueJwt: async ({}, use) => {
      await use(requireFixture().issueJwt);
    },
  });
  // biome-ignore-end lint/correctness/noEmptyPattern: Vitest fixture API requires this parameter structure

  const beforeAll = (fn: (ctx: FixtureContext) => Promise<void>) =>
    vitest.beforeAll(async () => {
      try {
        fixture = await buildFixture();
        await fn(fixture);
      } catch (error) {
        setupError = error;
        console.error("Fixture setup failed:", error);
        throw error;
      }
    });

  const afterAll = (fn: (ctx: FixtureContext) => Promise<void>) =>
    vitest.afterAll(async () => {
      if (!fixture) {
        return;
      }
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
