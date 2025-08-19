import type { MigrationInterface, QueryRunner } from "typeorm";

export class WorkloadDomain1755621623214 implements MigrationInterface {
  name = "WorkloadDomain1755621623214";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" ADD COLUMN "domain" character varying DEFAULT NULL`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" DROP COLUMN "domain"`,
    );
  }
}
