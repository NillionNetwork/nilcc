import { openAPISpecs } from "hono-openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";

export function createOpenApiRouter(options: ControllerOptions): void {
  const { app } = options;

  app.get(
    PathsV1.docs,
    openAPISpecs(app, {
      documentation: {
        info: {
          title: "nilcc-api",
          version: "0.1.0-beta.1",
          description: "API",
        },
      },
    }),
  );
}
