import { Column, Entity, OneToMany, PrimaryGeneratedColumn } from "typeorm";
import { WorkloadEntity } from "#/workload/workload.entity";

@Entity()
export class MetalInstanceEntity {
  @PrimaryGeneratedColumn("uuid")
  id: string;

  @Column({ type: "varchar" })
  hostname: string;

  @Column({ type: "varchar" })
  publicIp: string;

  @Column({ type: "varchar" })
  token: string;

  @Column({ type: "varchar" })
  agentVersion: string;

  @Column({ type: "int" })
  totalMemory: number;

  @Column({ type: "int" })
  osReservedMemory: number;

  @Column({ type: "int" })
  totalCpus: number;

  @Column({ type: "int" })
  osReservedCpus: number;

  @Column({ type: "int" })
  totalDisk: number;

  @Column({ type: "int" })
  osReservedDisk: number;

  @Column({ type: "int" })
  gpus: number;

  @Column({ type: "varchar", nullable: true })
  gpuModel?: string;

  @OneToMany(
    () => WorkloadEntity,
    (workload) => workload.metalInstance,
  )
  workloads: WorkloadEntity[];

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;

  @Column({ type: "timestamp" })
  lastSeenAt: Date;
}
