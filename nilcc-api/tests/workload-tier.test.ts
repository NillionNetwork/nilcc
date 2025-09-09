import { describe } from "vitest";
import type { CreateWorkloadTierRequest } from "#/workload-tier/workload-tier.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("WorkloadTier", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("should create a workload tier that hasn't been created", async ({
    expect,
    clients,
  }) => {
    const request: CreateWorkloadTierRequest = {
      name: "my favorite tier",
      cpus: 1,
      memoryMb: 1024,
      gpus: 2,
      diskGb: 12,
      cost: 10,
    };

    const tier = await clients.admin.createTier(request).submit();
    expect(tier.name).toBe(request.name);
    expect(tier.cpus).toBe(request.cpus);
    expect(tier.gpus).toBe(request.gpus);
    expect(tier.memoryMb).toBe(request.memoryMb);
    expect(tier.diskGb).toBe(request.diskGb);
    expect(tier.cost).toBe(request.cost);

    // Creating it again should fail
    expect(await clients.admin.createTier(request).status()).toBe(409);

    const tiers = await clients.user.listTiers().submit();
    expect(tiers).toEqual([tier]);

    await clients.admin.deleteTier(tier.tierId).submit();
    expect(await clients.user.listTiers().submit()).toEqual([]);
  });
});
