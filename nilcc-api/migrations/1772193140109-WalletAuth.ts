import type { MigrationInterface, QueryRunner } from "typeorm";

export class WalletAuth1772193140109 implements MigrationInterface {
  name = "WalletAuth1772193140109";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      "ALTER TABLE accounts ADD COLUMN wallet_address VARCHAR",
    );

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
    await queryRunner.query("ALTER TABLE accounts DROP COLUMN wallet_address");
  }
}
