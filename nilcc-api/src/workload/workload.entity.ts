import { Column, Entity, ManyToOne, OneToMany, PrimaryColumn } from "typeorm";
import { z } from "zod";
import { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

@Entity()
export class WorkloadEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @Column({ type: "varchar" })
  name: string;

  @Column({ type: "varchar", nullable: true })
  description?: string;

  @Column("simple-array", { nullable: true })
  tags?: string[];

  @Column({ type: "text" })
  dockerCompose: string;

  @Column({
    type: "text",
    nullable: true,
    transformer: {
      to: (value?: Record<string, string>) =>
        value
          ? z.record(z.string(), z.string()).parse(value) &&
            JSON.stringify(value)
          : null,
      from: (value?: string) => (value ? JSON.parse(value) : {}),
    },
  })
  envVars?: Record<string, string>;

  @Column({
    type: "text",
    nullable: true,
    transformer: {
      to: (value?: Record<string, string>) =>
        value
          ? z.record(z.string(), z.string()).parse(value) &&
            JSON.stringify(value)
          : null,
      from: (value?: string) => (value ? JSON.parse(value) : {}),
    },
  })
  files?: Record<string, string>;

  @Column({ type: "varchar" })
  serviceToExpose: string;

  @Column({ type: "int" })
  servicePortToExpose: number;

  @Column({ type: "int" })
  memory: number;

  @Column({ type: "int" })
  cpus: number;

  @Column({ type: "int" })
  gpus: number;

  @Column({ type: "int" })
  disk: number;

  @Column({ type: "varchar", default: "scheduled" })
  status: "scheduled" | "starting" | "running" | "stopped" | "error";

  @ManyToOne(
    () => MetalInstanceEntity,
    (metalInstance) => metalInstance.workloads,
  )
  metalInstance: MetalInstanceEntity;

  @OneToMany(
    () => WorkloadEventEntity,
    (events) => events.workload,
  )
  events: WorkloadEventEntity[];

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}

@Entity()
export class WorkloadEventEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @ManyToOne(
    () => WorkloadEntity,
    (workload) => workload.events,
    { onDelete: "CASCADE" },
  )
  workload: WorkloadEntity;

  @Column({ type: "varchar" })
  event: "created" | "starting" | "running" | "stopped" | "failedToStart";

  @Column({ type: "varchar", nullable: true })
  details?: string;

  @Column({ type: "timestamp" })
  timestamp: Date;
}
