import { Column, Entity, PrimaryColumn } from "typeorm";

@Entity({ name: "artifacts" })
export class ArtifactEntity {
  @PrimaryColumn({ type: "varchar" })
  version: string;

  @Column({ type: "timestamp" })
  builtAt: Date;
}
