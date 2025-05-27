import { Column, Entity, PrimaryGeneratedColumn } from "typeorm";

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

  @Column({ type: "varchar" })
  serviceToExpose: string;

  @Column({ type: "int" })
  servicePortToExpose: number;

  @Column({ type: "int" })
  memory: number;

  @Column({ type: "int" })
  cpu: number;

  @Column({
    type: "enum",
    enum: ["pending", "running", "stopped", "error"],
    default: "pending",
  })
  status: "pending" | "running" | "stopped" | "error";

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
