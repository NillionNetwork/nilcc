import type { MigrationInterface, QueryRunner } from "typeorm";

export class InitialState1754946297570 implements MigrationInterface {
  name = "InitialState1754946297570";

  public async up(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `CREATE TABLE "workload_entity" ("id" uuid NOT NULL, "name" character varying NOT NULL, "description" character varying, "tags" text, "dockerCompose" text NOT NULL, "envVars" text, "files" text, "serviceToExpose" character varying NOT NULL, "servicePortToExpose" integer NOT NULL, "memory" integer NOT NULL, "cpus" integer NOT NULL, "gpus" integer NOT NULL, "disk" integer NOT NULL, "status" character varying NOT NULL DEFAULT 'scheduled', "createdAt" TIMESTAMP NOT NULL, "updatedAt" TIMESTAMP NOT NULL, "metalInstanceId" uuid, CONSTRAINT "PK_4ab39b05ae550427c975bcb8918" PRIMARY KEY ("id"))`,
    );
    await queryRunner.query(
      `CREATE TABLE "workload_event_entity" ("id" uuid NOT NULL, "event" character varying NOT NULL, "details" character varying, "timestamp" TIMESTAMP NOT NULL, "workloadId" uuid, CONSTRAINT "PK_c6871be28c1582e2c9fb25b3371" PRIMARY KEY ("id"))`,
    );
    await queryRunner.query(
      `CREATE TABLE "metal_instance_entity" ("id" uuid NOT NULL, "hostname" character varying NOT NULL, "publicIp" character varying NOT NULL, "token" character varying NOT NULL, "agentVersion" character varying NOT NULL, "totalMemory" integer NOT NULL, "osReservedMemory" integer NOT NULL, "totalCpus" integer NOT NULL, "osReservedCpus" integer NOT NULL, "totalDisk" integer NOT NULL, "osReservedDisk" integer NOT NULL, "gpus" integer NOT NULL, "gpuModel" character varying, "createdAt" TIMESTAMP NOT NULL, "updatedAt" TIMESTAMP NOT NULL, "lastSeenAt" TIMESTAMP NOT NULL, CONSTRAINT "PK_8a9c95e60531975c99dc1859567" PRIMARY KEY ("id"))`,
    );
    await queryRunner.query(
      `ALTER TABLE "workload_entity" ADD CONSTRAINT "FK_8e17bda79146efc4778f1d96d11" FOREIGN KEY ("metalInstanceId") REFERENCES "metal_instance_entity"("id") ON DELETE NO ACTION ON UPDATE NO ACTION`,
    );
    await queryRunner.query(
      `ALTER TABLE "workload_event_entity" ADD CONSTRAINT "FK_2020172c2f6e609ec597f6fc215" FOREIGN KEY ("workloadId") REFERENCES "workload_entity"("id") ON DELETE CASCADE ON UPDATE NO ACTION`,
    );
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    await queryRunner.query(
      `ALTER TABLE "workload_event_entity" DROP CONSTRAINT "FK_2020172c2f6e609ec597f6fc215"`,
    );
    await queryRunner.query(
      `ALTER TABLE "workload_entity" DROP CONSTRAINT "FK_8e17bda79146efc4778f1d96d11"`,
    );
    await queryRunner.query(`DROP TABLE "metal_instance_entity"`);
    await queryRunner.query(`DROP TABLE "workload_event_entity"`);
    await queryRunner.query(`DROP TABLE "workload_entity"`);
  }
}
