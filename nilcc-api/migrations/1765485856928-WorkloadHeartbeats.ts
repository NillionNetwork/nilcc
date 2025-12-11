import type { MigrationInterface, QueryRunner } from "typeorm";

export class WorkloadHeartbeats1765485856928 implements MigrationInterface {
  name = "WorkloadHeartbeats1765485856928";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE workloads ADD COLUMN heartbeat TEXT DEFAULT NULL",
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("ALTER TABLE workloads DROP COLUMN heartbeat");
  }
}
