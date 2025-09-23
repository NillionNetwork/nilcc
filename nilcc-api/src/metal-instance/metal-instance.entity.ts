import { Column, Entity, OneToMany, PrimaryColumn } from "typeorm";
import z from "zod";
import { WorkloadEntity } from "#/workload/workload.entity";

@Entity({ name: "metal_instances" })
export class MetalInstanceEntity {
  @PrimaryColumn({ type: "uuid" })
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
  reservedMemory: number;

  @Column({ type: "int" })
  totalCpus: number;

  @Column({ type: "int" })
  reservedCpus: number;

  @Column({ type: "int" })
  totalDisk: number;

  @Column({ type: "int" })
  reservedDisk: number;

  @Column({ type: "int" })
  gpus: number;

  @Column({ type: "varchar", nullable: true })
  gpuModel?: string;

  @OneToMany(
    () => WorkloadEntity,
    (workload) => workload.metalInstance,
  )
  workloads: WorkloadEntity[];

  @Column({
    type: "text",
    transformer: {
      to: (value: string[]) => JSON.stringify(value),
      from: (value: string) => z.string().array().parse(JSON.parse(value)),
    },
  })
  availableArtifactVersions: string[];

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;

  @Column({ type: "timestamp" })
  lastSeenAt: Date;
}
