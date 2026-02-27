import { Column, Entity, ManyToOne, PrimaryColumn } from "typeorm";
import { AccountEntity } from "#/account/account.entity";

@Entity({ name: "payments" })
export class PaymentEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @Column({ type: "varchar", unique: true })
  txHash: string;

  @Column({ type: "int" })
  logIndex: number;

  @Column({ type: "int" })
  blockNumber: number;

  @Column({ type: "varchar" })
  fromAddress: string;

  @Column({ type: "varchar" })
  amount: string;

  @Column({ type: "varchar" })
  digest: string;

  @ManyToOne(() => AccountEntity)
  account: AccountEntity;

  @Column({ type: "int" })
  creditedAmount: number;

  @Column({ type: "timestamp" })
  createdAt: Date;
}
