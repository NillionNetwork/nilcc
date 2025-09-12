import type { MigrationInterface, QueryRunner } from "typeorm";

export class MetalInstanceIdNotNull1757713209092 implements MigrationInterface {
  name = "MetalInstanceIdNotNull1757713209092";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN metal_instance_id SET NOT NULL`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workloads" ALTER COLUMN metal_instance_id DROP NOT NULL`,
    );
  }
}
