import type { MigrationInterface, QueryRunner } from "typeorm";

export class Tiers1756136984989 implements MigrationInterface {
  name = "Tiers1756136984989";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `CREATE TABLE "workload_tier_entity" (id uuid NOT NULL, name character varying NOT NULL UNIQUE, cpus integer NOT NULL, gpus integer NOT NULL, memory integer NOT NULL, disk integer NOT NULL, cost integer NOT NULL, CONSTRAINT "PK_f62514bb6858ae5056e76107b8de58e7" PRIMARY KEY ("id"))`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(`DROP TABLE "workload_tier_entity"`);
  }
}
