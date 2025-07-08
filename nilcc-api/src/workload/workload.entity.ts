import { Column, Entity, ManyToOne, PrimaryGeneratedColumn } from "typeorm";
import { z } from "zod";
import { MetalInstanceEntity } from "#/metal-instance/metal-instance.entity";

@Entity()
export class WorkloadEntity {
  @PrimaryGeneratedColumn("uuid")
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

  @Column({
    type: "enum",
    enum: ["scheduled", "running", "stopped", "error"],
    default: "scheduled",
  })
  status: "scheduled" | "running" | "stopped" | "error";

  @ManyToOne(
    () => MetalInstanceEntity,
    (metalInstance) => metalInstance.workloads,
  )
  metalInstance: MetalInstanceEntity;

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
