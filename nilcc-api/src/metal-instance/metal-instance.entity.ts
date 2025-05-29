import { Column, Entity, PrimaryGeneratedColumn } from "typeorm";

@Entity()
export class MetalInstanceEntity {
  @PrimaryGeneratedColumn("uuid")
  id: string;

  @Column({ type: "varchar" })
  hostname: string;

  @Column({ type: "int" })
  memory: number;

  @Column({ type: "int" })
  cpu: number;

  @Column({ type: "int" })
  gpu?: number;

  @Column({ type: "varchar" })
  gpuModel?: string;

  @Column({ type: "varchar" })
  ipAddress: string;

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
