import { openAPISpecs } from "hono-openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { cors } from 'hono/cors'

export function createOpenApiRouter(options: ControllerOptions): void {
  const { app } = options;

  app.get(
    PathsV1.docs,
    cors(),
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
