import { Column, Entity, ManyToOne, PrimaryColumn } from "typeorm";
import { AccountEntity } from "#/account/account.entity";
import { bigintNumberTransformer } from "#/common/nil";

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

  @Column({ type: "float" })
  nilAmount: number;

  @Column({ type: "float" })
  nilPriceAtDeposit: number;

  @Column({
    type: "bigint",
    name: "deposited_amount_usd",
    transformer: bigintNumberTransformer,
  })
  depositedAmountUsd: number; // microdollars

  @Column({ type: "timestamp" })
  createdAt: Date;
}
