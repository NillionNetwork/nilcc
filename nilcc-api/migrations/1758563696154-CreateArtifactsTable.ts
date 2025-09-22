import type { MigrationInterface, QueryRunner } from "typeorm";

export class CreateArtifactsTable1758563696154 implements MigrationInterface {
  name = "CreateArtifactsTable1758563696154";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "CREATE TABLE artifacts (version character varying NOT NULL PRIMARY KEY, built_at TIMESTAMP NOT NULL)",
    );
    await queryRunner.query(
      "ALTER TABLE workloads ADD COLUMN artifacts_version character varying DEFAULT NULL",
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("DROP TABLE artifacts");
    await queryRunner.query(
      "ALTER TABLE workloads DROP COLUMN artifacts_version",
    );
  }
}
