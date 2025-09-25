import { z } from "zod";

export const Artifact = z
  .object({
    version: z.string().openapi({ description: "The artifact version." }),
    builtAt: z
      .string()
      .datetime()
      .openapi({ description: "The timestamp when this artifact was built." }),
  })
  .openapi({ ref: "Artifact" });
export type Artifact = z.infer<typeof Artifact>;

export const EnableArtifactRequest = z
  .object({
    version: z
      .string()
      .openapi({ description: "The artifact version to be enabled." }),
  })
  .openapi({ ref: "EnableArtifactRequest" });
export type EnableArtifactRequest = z.infer<typeof EnableArtifactRequest>;

export const DeleteArtifactRequest = z
  .object({
    version: z.string().openapi({
      description: "The artifact version to be deleted.",
    }),
  })
  .openapi({ ref: "DeleteArtifactRequest" });
export type DeleteArtifactRequest = z.infer<typeof DeleteArtifactRequest>;
