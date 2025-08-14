import type { MigrationInterface, QueryRunner } from "typeorm";

export class WorkloadAccount1755195024670 implements MigrationInterface {
  name = "WorkloadAccount1755195024670";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `CREATE UNIQUE INDEX account_entity_api_token_idx ON account_entity ("apiToken")`,
    );
    await queryRunner.query(
      `ALTER TABLE workload_entity ADD COLUMN "accountId" character varying NOT NULL`,
    );
    await queryRunner.query(
      `ALTER TABLE "workload_entity" ADD CONSTRAINT "FK_819ed6ec38375dfa30487223b01" FOREIGN KEY ("accountId") REFERENCES "account_entity"("id") ON DELETE NO ACTION ON UPDATE NO ACTION`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query("DROP INDEX account_entity_api_token_idx");
    await queryRunner.query(
      `ALTER TABLE workload_entity DROP COLUMN "accountId"`,
    );
    await queryRunner.query(
      `ALTER TABLE "workload_entity" DROP CONSTRAINT "FK_819ed6ec38375dfa30487223b01"`,
    );
  }
}
