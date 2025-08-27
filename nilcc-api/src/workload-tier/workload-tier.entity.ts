import { Column, Entity, PrimaryColumn } from "typeorm";

@Entity({ name: "workload_tiers" })
export class WorkloadTierEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @Column({ type: "varchar", unique: true })
  name: string;

  @Column({ type: "int" })
  memory: number;

  @Column({ type: "int" })
  cpus: number;

  @Column({ type: "int" })
  gpus: number;

  @Column({ type: "int" })
  disk: number;

  @Column({ type: "int" })
  cost: number;
}
