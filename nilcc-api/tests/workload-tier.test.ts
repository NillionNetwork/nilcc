import { describe } from "vitest";
import type {
  CreateWorkloadTierRequest,
  UpdateWorkloadTierRequest,
} from "#/workload-tier/workload-tier.dto";
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
    expect(await clients.admin.listTiers().submit()).toEqual([]);
  });

  it("should update an existing workload tier", async ({ expect, clients }) => {
    const created = await clients.admin
      .createTier({
        name: "tier-to-update",
        cpus: 1,
        memoryMb: 1024,
        gpus: 0,
        diskGb: 10,
        cost: 1,
      })
      .submit();

    const request: UpdateWorkloadTierRequest = {
      tierId: created.tierId,
      name: "tier-updated",
      cpus: 4,
      memoryMb: 8192,
      gpus: 1,
      diskGb: 40,
      cost: 7.5,
    };

    const updated = await clients.admin.updateTier(request).submit();

    expect(updated).toEqual({
      tierId: created.tierId,
      name: request.name,
      cpus: request.cpus,
      memoryMb: request.memoryMb,
      gpus: request.gpus,
      diskGb: request.diskGb,
      cost: request.cost,
    });
    expect(await clients.user.listTiers().submit()).toEqual([updated]);
  });
});
