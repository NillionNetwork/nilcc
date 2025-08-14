import { Column, Entity, OneToMany, PrimaryColumn } from "typeorm";
import { WorkloadEntity } from "#/workload/workload.entity";

@Entity()
export class AccountEntity {
  @PrimaryColumn({ type: "varchar" })
  id: string;

  @Column({ type: "varchar", unique: true })
  name: string;

  @Column({ type: "varchar", unique: true })
  apiToken: string;

  @OneToMany(
    () => WorkloadEntity,
    (workload) => workload.account,
  )
  workloads: WorkloadEntity[];

  @Column({ type: "timestamp" })
  createdAt: Date;
}
