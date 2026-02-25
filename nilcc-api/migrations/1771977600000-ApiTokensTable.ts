import type { MigrationInterface, QueryRunner } from "typeorm";

export class ApiTokensTable1771977600000 implements MigrationInterface {
  name = "ApiTokensTable1771977600000";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`
      CREATE TABLE api_tokens (
        id            character varying NOT NULL,
        token         character varying NOT NULL,
        account_id    character varying NOT NULL,
        created_at    TIMESTAMP NOT NULL,
        CONSTRAINT pk_api_tokens PRIMARY KEY (id)
      )
    `);
    await queryRunner.query(
      "CREATE UNIQUE INDEX api_tokens_token_idx ON api_tokens (token)",
    );
    await queryRunner.query(`
      ALTER TABLE api_tokens
        ADD CONSTRAINT fk_api_tokens_account_id
        FOREIGN KEY (account_id) REFERENCES accounts(id)
    `);
    await queryRunner.query(`
      INSERT INTO api_tokens (id, token, created_at)
      SELECT gen_random_uuid()::text, api_token, NOW()
      FROM accounts
    `);
    await queryRunner.query("DROP INDEX account_entity_api_token_idx");
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE accounts ALTER COLUMN api_token SET NOT NULL",
    );
    await queryRunner.query(
      "CREATE UNIQUE INDEX account_entity_api_token_idx ON accounts (api_token)",
    );
    await queryRunner.query(
      "ALTER TABLE api_tokens DROP CONSTRAINT fk_api_tokens_account_id",
    );
  }
}
