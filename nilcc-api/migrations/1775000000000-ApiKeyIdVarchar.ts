import type { MigrationInterface, QueryRunner } from "typeorm";

const UUID_PATTERN =
  "^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$";

export class ApiKeyIdVarchar1775000000000 implements MigrationInterface {
  name = "ApiKeyIdVarchar1775000000000";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`
      ALTER TABLE api_keys
      ALTER COLUMN id TYPE VARCHAR USING id::VARCHAR
    `);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`
      DO $$
      BEGIN
        IF EXISTS (
          SELECT 1
          FROM api_keys
          WHERE id !~* '${UUID_PATTERN}'
        ) THEN
          RAISE EXCEPTION 'cannot narrow api_keys.id back to UUID while non-UUID API keys exist';
        END IF;
      END
      $$;
    `);

    await queryRunner.query(`
      ALTER TABLE api_keys
      ALTER COLUMN id TYPE UUID USING id::UUID
    `);
  }
}
