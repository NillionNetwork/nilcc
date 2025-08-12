import type { MigrationInterface, QueryRunner } from "typeorm";

export class Account1755033746208 implements MigrationInterface {
  name = "Account1755033746208";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `CREATE TABLE "account_entity" ("id" character varying NOT NULL, "name" character varying NOT NULL, "apiToken" character varying NOT NULL, "createdAt" TIMESTAMP NOT NULL, CONSTRAINT "UQ_8caa8ac488d153a508cdd657011" UNIQUE ("name"), CONSTRAINT "PK_b482dad15becff9a89ad707dcbe" PRIMARY KEY ("id"))`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`DROP TABLE "account_entity"`);
  }
}
