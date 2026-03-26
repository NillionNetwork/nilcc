import type { MigrationInterface, QueryRunner } from "typeorm";

export class WalletAuth1773000000000 implements MigrationInterface {
  name = "WalletAuth1773000000000";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE accounts ADD COLUMN wallet_address VARCHAR",
    );

    // Migrate existing api_token values into api_keys before dropping the column.
    // Uses the api_token as the api_key id so existing clients can continue using it.
    await queryRunner.query(`
      INSERT INTO api_keys (id, account_id, type, active, created_at, updated_at)
      SELECT
        api_token,
        id,
        'account-admin',
        true,
        NOW(),
        NOW()
      FROM accounts
      WHERE api_token IS NOT NULL
    `);

    await queryRunner.query("ALTER TABLE accounts DROP COLUMN api_token");

    await queryRunner.query(
      "CREATE UNIQUE INDEX idx_accounts_wallet_address ON accounts(wallet_address)",
    );

    await queryRunner.query(`
      CREATE TABLE auth_nonces (
        id UUID PRIMARY KEY,
        wallet_address VARCHAR NOT NULL,
        expires_at TIMESTAMP NOT NULL,
        created_at TIMESTAMP NOT NULL
      )
    `);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("DROP TABLE auth_nonces");
    await queryRunner.query("DROP INDEX idx_accounts_wallet_address");
    await queryRunner.query(
      "ALTER TABLE accounts ADD COLUMN api_token VARCHAR UNIQUE",
    );
    // Restore api_token from migrated api_keys
    await queryRunner.query(`
      UPDATE accounts SET api_token = ak.id::VARCHAR
      FROM (
        SELECT DISTINCT ON (account_id) id, account_id
        FROM api_keys
        WHERE type = 'account-admin'
        ORDER BY account_id, created_at ASC
      ) ak
      WHERE accounts.id = ak.account_id
    `);
    await queryRunner.query("ALTER TABLE accounts DROP COLUMN wallet_address");
  }
}
