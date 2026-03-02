import { Column, Entity, OneToMany, PrimaryColumn } from "typeorm";
import { ApiKeyEntity } from "#/api-key/api-key.entity";
import { WorkloadEntity } from "#/workload/workload.entity";

@Entity({ name: "accounts" })
export class AccountEntity {
  @PrimaryColumn({ type: "varchar" })
  id: string;

  @Column({ type: "varchar", unique: true })
  name: string;

  @Column({ type: "varchar", unique: true })
  walletAddress: string;

  @Column({ type: "int" })
  credits: number;

  @OneToMany(
    () => WorkloadEntity,
    (workload) => workload.account,
  )
  workloads: WorkloadEntity[];

  @OneToMany(
    () => ApiKeyEntity,
    (apiKey) => apiKey.account,
  )
  apiKeys: ApiKeyEntity[];

  @Column({ type: "timestamp" })
  createdAt: Date;
}
