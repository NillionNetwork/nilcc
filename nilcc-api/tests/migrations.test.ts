import type { QueryRunner } from "typeorm";
import { describe, expect, it, vi } from "vitest";
import { UsdBasedPricing1774000000000 } from "../migrations/1774000000000-UsdBasedPricing";

describe("UsdBasedPricing1774000000000", () => {
  it("backfills deposited USD from credited amounts", async () => {
    const queryRunner = {
      query: vi.fn().mockResolvedValue(undefined),
    } as unknown as QueryRunner;

    await new UsdBasedPricing1774000000000().up(queryRunner);

    expect(queryRunner.query).toHaveBeenCalledWith(
      expect.stringContaining(
        'SET "deposited_amount_usd" = CAST("credited_amount" AS bigint) * 1000000',
      ),
    );
  });
});
