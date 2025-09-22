import type { Artifact } from "./artifact.dto";
import type { ArtifactEntity } from "./artifact.entity";

export const artifactMapper = {
  entityToResponse(artifact: ArtifactEntity): Artifact {
    return {
      version: artifact.version,
      builtAt: artifact.builtAt.toISOString(),
    };
  },
};
