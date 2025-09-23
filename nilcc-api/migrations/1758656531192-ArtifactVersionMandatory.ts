import type { MigrationInterface, QueryRunner } from "typeorm";

export class ArtifactVersionMandatory1758656531192
  implements MigrationInterface
{
  name = "ArtifactVersionMandatory1758656531192";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE workloads ALTER COLUMN artifacts_version SET NOT NULL",
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE workloads ALTER COLUMN artifacts_version DROP NOT NULL",
    );
  }
}
