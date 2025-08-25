import type { MigrationInterface, QueryRunner } from "typeorm";

export class WorkloadCreditRate1756151064011 implements MigrationInterface {
  name = "WorkloadCreditRate1756151064011";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" ADD COLUMN "creditRate" integer NOT NULL DEFAULT 0`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" DROP COLUMN "creditRate"`,
    );
  }
}
