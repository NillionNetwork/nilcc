import type { MigrationInterface, QueryRunner } from "typeorm";

export class NilBasedPricing1774000000000 implements MigrationInterface {
  public async up(queryRunner: QueryRunner): Promise<void> {
    // Convert accounts.credits (int, 1 credit = 1 NIL) to accounts.balance (double precision, decimal NIL)
    await queryRunner.query(
      `ALTER TABLE "accounts" ADD "balance" double precision NOT NULL DEFAULT 0`,
    );
    await queryRunner.query(`UPDATE "accounts" SET "balance" = "credits"`);
    await queryRunner.query(`ALTER TABLE "accounts" DROP COLUMN "credits"`);

    // Rename workloads.credit_rate to usd_cost_per_min and change type to double precision
    await queryRunner.query(
      `ALTER TABLE "workloads" RENAME COLUMN "credit_rate" TO "usd_cost_per_min"`,
    );
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN "usd_cost_per_min" TYPE double precision`,
    );

    // Change workload_tiers.cost type from int to double precision
    await queryRunner.query(
      `ALTER TABLE "workload_tiers" ALTER COLUMN "cost" TYPE double precision`,
    );

    // Convert payments.credited_amount (int) to deposited_amount (double precision, decimal NIL)
    // The existing amount column stores the uint256 value; deposited_amount = amount / 10^6
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "deposited_amount" double precision`,
    );
    await queryRunner.query(
      `UPDATE "payments" SET "deposited_amount" = CAST("amount" AS double precision) / 1000000`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" ALTER COLUMN "deposited_amount" SET NOT NULL`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" DROP COLUMN "credited_amount"`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    // Restore payments.credited_amount from deposited_amount
    await queryRunner.query(
      `ALTER TABLE "payments" ADD "credited_amount" integer`,
    );
    await queryRunner.query(
      `UPDATE "payments" SET "credited_amount" = CAST("deposited_amount" * 1000 AS integer)`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" ALTER COLUMN "credited_amount" SET NOT NULL`,
    );
    await queryRunner.query(
      `ALTER TABLE "payments" DROP COLUMN "deposited_amount"`,
    );

    // Restore workload_tiers.cost type to int
    await queryRunner.query(
      `ALTER TABLE "workload_tiers" ALTER COLUMN "cost" TYPE integer`,
    );

    // Restore workloads.usd_cost_per_min back to credit_rate as int
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN "usd_cost_per_min" TYPE integer`,
    );
    await queryRunner.query(
      `ALTER TABLE "workloads" RENAME COLUMN "usd_cost_per_min" TO "credit_rate"`,
    );

    // Restore accounts.credits from balance
    await queryRunner.query(
      `ALTER TABLE "accounts" ADD "credits" integer NOT NULL DEFAULT 0`,
    );
    await queryRunner.query(
      `UPDATE "accounts" SET "credits" = CAST("balance" AS integer)`,
    );
    await queryRunner.query(`ALTER TABLE "accounts" DROP COLUMN "balance"`);
  }
}
