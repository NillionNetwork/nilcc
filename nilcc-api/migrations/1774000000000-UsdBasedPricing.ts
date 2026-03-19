import type { MigrationInterface, QueryRunner } from "typeorm";

export class UsdBasedPricing1774000000000 implements MigrationInterface {
  public async up(queryRunner: QueryRunner): Promise<void> {
    // Convert accounts.credits (int) to accounts.balance (bigint, microdollars: 1 USD = 1,000,000)
    await queryRunner.query(
      `ALTER TABLE "accounts" ADD "balance" bigint NOT NULL DEFAULT 0`,
    );
    await queryRunner.query(
      `UPDATE "accounts" SET "balance" = "credits" * 1000000`,
    );
    await queryRunner.query(`ALTER TABLE "accounts" DROP COLUMN "credits"`);

    // Rename workloads.credit_rate to usd_cost_per_min and change type to bigint (microdollars)
    await queryRunner.query(
      `ALTER TABLE "workloads" RENAME COLUMN "credit_rate" TO "usd_cost_per_min"`,
    );
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN "usd_cost_per_min" TYPE bigint USING "usd_cost_per_min" * 1000000`,
    );

    // Change workload_tiers.cost type from int to bigint (microdollars)
    await queryRunner.query(
      `ALTER TABLE "workload_tiers" ALTER COLUMN "cost" TYPE bigint USING "cost" * 1000000`,
    );

    // Replace payments.credited_amount with USD-based columns and audit fields
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "nil_amount" double precision NOT NULL DEFAULT 0`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "nil_price_at_deposit" double precision NOT NULL DEFAULT 1.0`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "deposited_amount_usd" bigint NOT NULL DEFAULT 0`,
    );
    // Backfill from existing credited balances so migrated payment history stays
    // consistent with migrated account balances.
    await queryRunner.query(
      `UPDATE "payments" SET "nil_amount" = CAST("amount" AS double precision) / 1000000`,
    );
    await queryRunner.query(
      `UPDATE "payments" SET "nil_price_at_deposit" = CASE
        WHEN CAST("amount" AS double precision) = 0 THEN 1.0
        ELSE CAST("credited_amount" AS double precision) / (CAST("amount" AS double precision) / 1000000)
      END`,
    );
    await queryRunner.query(
      `UPDATE "payments" SET "deposited_amount_usd" = CAST("credited_amount" AS bigint) * 1000000`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" DROP COLUMN "credited_amount"`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    // Restore payments.credited_amount
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "credited_amount" integer`,
    );
    await queryRunner.query(
      `UPDATE "payments" SET "credited_amount" = CAST("nil_amount" * 1000 AS integer)`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" ALTER COLUMN "credited_amount" SET NOT NULL`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" DROP COLUMN "deposited_amount_usd"`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" DROP COLUMN "nil_price_at_deposit"`,
    );
    await queryRunner.query(`ALTER TABLE "payments" DROP COLUMN "nil_amount"`);

    // Restore workload_tiers.cost type to int (divide by 1,000,000)
    await queryRunner.query(
      `ALTER TABLE "workload_tiers" ALTER COLUMN "cost" TYPE integer USING ("cost" / 1000000)::integer`,
    );

    // Restore workloads.usd_cost_per_min back to credit_rate as int
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN "usd_cost_per_min" TYPE integer USING ("usd_cost_per_min" / 1000000)::integer`,
    );
    await queryRunner.query(
      `ALTER TABLE "workloads" RENAME COLUMN "usd_cost_per_min" TO "credit_rate"`,
    );

    // Restore accounts.credits from balance
    await queryRunner.query(
      `ALTER TABLE "accounts" ADD "credits" integer NOT NULL DEFAULT 0`,
    );
    await queryRunner.query(
      `UPDATE "accounts" SET "credits" = ("balance" / 1000000)::integer`,
    );
    await queryRunner.query(`ALTER TABLE "accounts" DROP COLUMN "balance"`);
  }
}
