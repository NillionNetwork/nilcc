import type { MigrationInterface, QueryRunner } from "typeorm";

const TABLE_NAMES: string[][] = [
  ["account_entity", "accounts"],
  ["metal_instance_entity", "metal_instances"],
  ["workload_entity", "workloads"],
  ["workload_event_entity", "workload_events"],
  ["workload_tier_entity", "workload_tiers"],
];

const TABLE_FIELDS: Record<string, string[][]> = {
  accounts: [
    ["apiToken", "api_token"],
    ["createdAt", "created_at"],
  ],
  metal_instances: [
    ["publicIp", "public_ip"],
    ["agentVersion", "agent_version"],
    ["totalMemory", "total_memory"],
    ["osReservedMemory", "reserved_memory"],
    ["osReservedCpus", "reserved_cpus"],
    ["totalCpus", "total_cpus"],
    ["totalDisk", "total_disk"],
    ["osReservedDisk", "reserved_disk"],
    ["gpuModel", "gpu_model"],
    ["createdAt", "created_at"],
    ["updatedAt", "updated_at"],
    ["lastSeenAt", "last_seen_at"],
  ],
  workloads: [
    ["accountId", "account_id"],
    ["dockerCompose", "docker_compose"],
    ["envVars", "env_vars"],
    ["dockerCredentials", "docker_credentials"],
    ["serviceToExpose", "public_container_name"],
    ["servicePortToExpose", "public_container_port"],
    ["creditRate", "credit_rate"],
    ["metalInstanceId", "metal_instance_id"],
    ["createdAt", "created_at"],
    ["updatedAt", "updated_at"],
  ],
  workload_events: [["workloadId", "workload_id"]],
};

export class BigRename1756308586154 implements MigrationInterface {
  name = "BigRename1756308586154";

  public async up(queryRunner: QueryRunner): Promise<void> {
    for (const [from, to] of TABLE_NAMES) {
      await queryRunner.query(`ALTER TABLE ${from} RENAME TO ${to}`);
    }
    for (const [table, changes] of Object.entries(TABLE_FIELDS)) {
      for (const [from, to] of changes) {
        await queryRunner.query(
          `ALTER TABLE ${table} RENAME COLUMN "${from}" TO "${to}"`,
        );
      }
    }
  }

  public async down(queryRunner: QueryRunner): Promise<void> {
    for (const [to, from] of TABLE_NAMES) {
      await queryRunner.query(`ALTER TABLE ${from} RENAME TO ${to}`);
    }
    for (const [table, changes] of Object.entries(TABLE_FIELDS)) {
      for (const [to, from] of changes) {
        await queryRunner.query(
          `ALTER TABLE ${table} RENAME COLUMN "${from}" TO "${to}"`,
        );
      }
    }
  }
}
