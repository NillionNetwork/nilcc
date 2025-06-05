import { Column, Entity, OneToMany, PrimaryGeneratedColumn } from "typeorm";
import { WorkloadEntity } from "#/workload/workload.entity";

@Entity()
export class MetalInstanceEntity {
  @PrimaryGeneratedColumn("uuid")
  id: string;

  @Column({ type: "varchar" })
  hostname: string;

  @Column({ type: "varchar" })
  agentVersion: string;

  @Column({ type: "int" })
  memory: number;

  @Column({ type: "int" })
  cpu: number;

  @Column({ type: "int" })
  disk: number;

  @Column({ type: "int", nullable: true })
  gpu?: number;

  @Column({ type: "varchar", nullable: true })
  gpuModel?: string;

  @Column({ type: "varchar" })
  ipAddress: string;

  @OneToMany(
    () => WorkloadEntity,
    (workload) => workload.metalInstance,
  )
  workloads: WorkloadEntity[];

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
