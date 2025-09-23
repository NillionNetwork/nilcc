import type { MigrationInterface, QueryRunner } from "typeorm";

export class MetalInstanceArtifactVersions1758645450546
  implements MigrationInterface
{
  name = "MetalInstanceArtifactVersions1758645450546";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE metal_instances ADD COLUMN available_artifact_versions TEXT NOT NULL DEFAULT '[]'",
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE metal_instances DROP COLUMN available_artifact_versions",
    );
  }
}
