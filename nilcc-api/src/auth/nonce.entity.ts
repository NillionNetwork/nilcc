import { Column, Entity, PrimaryColumn } from "typeorm";

@Entity({ name: "auth_nonces" })
export class NonceEntity {
  @PrimaryColumn({ type: "uuid" })
  id: string;

  @Column({ type: "varchar" })
  walletAddress: string;

  @Column({ type: "timestamp" })
  expiresAt: Date;

  @Column({ type: "timestamp" })
  createdAt: Date;
}
