import { Column, Entity, PrimaryColumn } from "typeorm";

@Entity({ name: "block_cursors" })
export class BlockCursorEntity {
  @PrimaryColumn({ type: "varchar" })
  id: string;

  @Column({ type: "bigint" })
  lastProcessedBlock: string;

  @Column({ type: "timestamp" })
  updatedAt: Date;
}
