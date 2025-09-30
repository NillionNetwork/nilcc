import type { ControllerOptions } from "#/common/types";
import * as ArtifactController from "./artifact.controller";

export function buildArtifactRouter(options: ControllerOptions): void {
  ArtifactController.enable(options);
  ArtifactController.list(options);
  ArtifactController.disable(options);
}
