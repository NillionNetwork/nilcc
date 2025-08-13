import { Column, Entity, PrimaryColumn } from "typeorm";

@Entity()
export class AccountEntity {
  @PrimaryColumn({ type: "varchar" })
  id: string;

  @Column({ type: "varchar", unique: true })
  name: string;

  @Column({ type: "varchar" })
  apiToken: string;

  @Column({ type: "timestamp" })
  createdAt: Date;
}
