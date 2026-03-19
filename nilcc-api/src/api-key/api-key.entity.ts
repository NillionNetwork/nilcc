import {
  Column,
  Entity,
  JoinColumn,
  ManyToOne,
  PrimaryColumn,
  type Relation,
} from "typeorm";
import { AccountEntity } from "#/account/account.entity";
import type { ApiKeyType } from "./api-key.dto";

@Entity({ name: "api_keys" })
export class ApiKeyEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @Column({ type: "varchar" })
  accountId: string;

  @ManyToOne(() => AccountEntity, { nullable: false, onDelete: "CASCADE" })
  @JoinColumn({ name: "account_id" })
  account: Relation<AccountEntity>;

  @Column({ type: "varchar" })
  type: ApiKeyType;

  @Column({ type: "boolean", default: true })
  active: boolean;

  @Column({ type: "timestamp" })
  createdAt: Date;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
