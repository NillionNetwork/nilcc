import type { MigrationInterface, QueryRunner } from "typeorm";

export class DockerCredentials1755638110882 implements MigrationInterface {
  name = "DockerCredentials1755638110882";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" ADD "dockerCredentials" text DEFAULT NULL`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_entity" DROP COLUMN "dockerCredentials"`,
    );
  }
}
