import type { MigrationInterface, QueryRunner } from "typeorm";

export class ApiKeys1772193140110 implements MigrationInterface {
  name = "ApiKeys1772193140110";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`
      CREATE TABLE api_keys (
        id VARCHAR PRIMARY KEY,
        account_id VARCHAR NOT NULL,
        type VARCHAR NOT NULL,
        active BOOLEAN NOT NULL DEFAULT true,
        created_at TIMESTAMP NOT NULL,
        updated_at TIMESTAMP NOT NULL,
        CONSTRAINT fk_api_keys_account_id FOREIGN KEY (account_id)
          REFERENCES accounts(id)
          ON DELETE CASCADE,
        CONSTRAINT chk_api_keys_type CHECK (type IN ('account-admin', 'user'))
      )
    `);

    await queryRunner.query(
      "CREATE INDEX idx_api_keys_account_id ON api_keys(account_id)",
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("DROP INDEX idx_api_keys_account_id");
    await queryRunner.query("DROP TABLE api_keys");
  }
}
