import { Column, Entity, ManyToOne, PrimaryColumn } from "typeorm";
import { AccountEntity } from "#/account/account.entity";

@Entity({ name: "api_tokens" })
export class ApiTokenEntity {
  @PrimaryColumn({ type: "varchar" })
  id: string;

  @Column({ type: "varchar", unique: true })
  token: string;

  @ManyToOne(() => AccountEntity, { onDelete: "CASCADE" })
  account: AccountEntity;

  @Column({ type: "timestamp" })
  createdAt: Date;
}
