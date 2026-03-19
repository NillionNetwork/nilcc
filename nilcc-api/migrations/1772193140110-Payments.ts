import type { MigrationInterface, QueryRunner } from "typeorm";

export class Payments1772193140110 implements MigrationInterface {
  name = "Payments1772193140110";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`
      CREATE TABLE payments (
        id UUID PRIMARY KEY,
        tx_hash VARCHAR NOT NULL UNIQUE,
        log_index INT NOT NULL,
        block_number INT NOT NULL,
        from_address VARCHAR NOT NULL,
        amount VARCHAR NOT NULL,
        digest VARCHAR NOT NULL,
        account_id VARCHAR NOT NULL REFERENCES accounts(id),
        credited_amount INT NOT NULL,
        created_at TIMESTAMP NOT NULL
      )
    `);

    await queryRunner.query(
      "CREATE INDEX idx_payments_from_address ON payments(from_address)",
    );
    await queryRunner.query(
      "CREATE INDEX idx_payments_account_id ON payments(account_id)",
    );

    await queryRunner.query(`
      CREATE TABLE block_cursors (
        id VARCHAR PRIMARY KEY,
        last_processed_block BIGINT NOT NULL,
        updated_at TIMESTAMP NOT NULL
      )
    `);
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("DROP TABLE block_cursors");
    await queryRunner.query("DROP TABLE payments");
  }
}
