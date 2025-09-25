import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import semver, { SemVer } from "semver";
import { adminAuthentication, userAuthentication } from "#/common/auth";
import {
  OpenApiSpecCommonErrorResponses,
  OpenApiSpecEmptySuccessResponses,
} from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator } from "#/common/zod-utils";
import {
  Artifact,
  DeleteArtifactRequest,
  EnableArtifactRequest,
} from "./artifact.dto";
import type { ArtifactEntity } from "./artifact.entity";
import { artifactMapper } from "./artifact.mapper";

export function enable(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.artifacts.enable,
    describeRoute({
      tags: ["artifact"],
      summary: "Enable an artifact version",
      description:
        "This will enable an artifact so it can be used by any new workloads.",
      responses: {
        200: {
          description: "Artifact enabled successfully",
          content: {
            "application/json": {
              schema: resolver(Artifact),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(EnableArtifactRequest),
    async (c) => {
      const payload = c.req.valid("json");
      const account = await bindings.services.artifact.enable(
        bindings,
        payload,
      );
      return c.json(artifactMapper.entityToResponse(account));
    },
  );
}

export function list(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.artifacts.list,
    describeRoute({
      tags: ["artifact"],
      summary: "List all available artifact versions.",
      description: "This lists all artifact versions.",
      responses: {
        200: {
          description: "The list of artifact versions.",
          content: {
            "application/json": {
              schema: resolver(Artifact.array()),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    async (c) => {
      const accounts = await bindings.services.artifact.list(bindings);
      const makeSortable = (artifact: ArtifactEntity): number | SemVer => {
        const parsed = semver.parse(artifact.version);
        if (parsed !== null) {
          return parsed;
        }
        return artifact.builtAt.getTime();
      };
      // Sort them so semvers appear first (highest semver first), then after that non semver sorted by build time.
      accounts.sort((a, b) => {
        const left = makeSortable(a);
        const right = makeSortable(b);
        if (left instanceof SemVer) {
          if (right instanceof SemVer) {
            // Both semver => compare them
            return right.compare(left);
          }
          // Only left is semver, it should go first
          return -1;
        }

        if (right instanceof SemVer) {
          // Only right is semver, it should go first
          return 1;
        }
        return right - left;
      });
      return c.json(accounts.map(artifactMapper.entityToResponse));
    },
  );
}

export function remove(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.post(
    PathsV1.artifacts.delete,
    describeRoute({
      tags: ["artifacts"],
      summary: "Delete a supported artifact version",
      description:
        "This will delete a supported artifact version. Any workload that is already running using the deleted version will continue to do so.",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(DeleteArtifactRequest),
    async (c) => {
      const version = c.req.valid("json").version;
      await bindings.services.artifact.remove(bindings, version);
      return c.json({});
    },
  );
}
