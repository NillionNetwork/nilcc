import type { QueryRunner } from "typeorm";
import { describe, expect, it, vi } from "vitest";
import { WalletAuth1773000000000 } from "../migrations/1773000000000-WalletAuth";
import { UsdBasedPricing1774000000000 } from "../migrations/1774000000000-UsdBasedPricing";
import { ApiKeyIdVarchar1775000000000 } from "../migrations/1775000000000-ApiKeyIdVarchar";

describe("UsdBasedPricing1774000000000", () => {
  it("backfills deposited USD from credited amounts", async () => {
    const queryRunner = {
      query: vi.fn().mockResolvedValue(undefined),
    } as unknown as QueryRunner;

    await new UsdBasedPricing1774000000000().up(queryRunner);

    expect(queryRunner.query).toHaveBeenCalledWith(
      expect.stringContaining(
        'SET "deposited_amount_usd" = COALESCE("credited_amount", 0)::bigint * 1000000::bigint',
      ),
    );
  });
});

describe("WalletAuth1773000000000", () => {
  it("migrates all legacy api_token values without UUID filtering", async () => {
    const queryRunner = {
      query: vi.fn().mockResolvedValue(undefined),
    } as unknown as QueryRunner;

    await new WalletAuth1773000000000().up(queryRunner);

    const insertCall = vi
      .mocked(queryRunner.query)
      .mock.calls.find(([sql]) => sql.includes("INSERT INTO api_keys"));

    expect(insertCall?.[0]).toContain("api_token,");
    expect(insertCall?.[0]).toContain("WHERE api_token IS NOT NULL");
    expect(insertCall?.[0]).not.toContain("CAST(api_token AS UUID)");
    expect(insertCall?.[0]).not.toContain("api_token ~");
  });
});

describe("ApiKeyIdVarchar1775000000000", () => {
  it("widens api_keys.id to varchar in up migration", async () => {
    const queryRunner = {
      query: vi.fn().mockResolvedValue(undefined),
    } as unknown as QueryRunner;

    await new ApiKeyIdVarchar1775000000000().up(queryRunner);

    expect(queryRunner.query).toHaveBeenCalledWith(
      expect.stringContaining("ALTER COLUMN id TYPE VARCHAR USING id::VARCHAR"),
    );
  });

  it("guards non-UUID keys before narrowing on down migration", async () => {
    const queryRunner = {
      query: vi.fn().mockResolvedValue(undefined),
    } as unknown as QueryRunner;

    await new ApiKeyIdVarchar1775000000000().down(queryRunner);

    expect(queryRunner.query).toHaveBeenNthCalledWith(
      1,
      expect.stringContaining("cannot narrow api_keys.id back to UUID"),
    );
    expect(queryRunner.query).toHaveBeenNthCalledWith(
      2,
      expect.stringContaining("ALTER COLUMN id TYPE UUID USING id::UUID"),
    );
  });
});
