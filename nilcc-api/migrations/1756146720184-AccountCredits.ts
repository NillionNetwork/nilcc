import type { MigrationInterface, QueryRunner } from "typeorm";

export class AccountCredits1756146720184 implements MigrationInterface {
  name = "AccountCredits1756146720184";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "account_entity" ADD COLUMN "credits" integer NOT NULL DEFAULT 0`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "account_entity" DROP COLUMN "credits"`,
    );
  }
}
