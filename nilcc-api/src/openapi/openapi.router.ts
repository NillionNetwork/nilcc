import { openAPISpecs } from "hono-openapi";
import type { ControllerOptions } from "#/common/types";

export function createOpenApiRouter(options: ControllerOptions): void {
  const { app } = options;

  app.get(
    "/openapi.json",
    openAPISpecs(app, {
      documentation: {
        info: {
          title: "nilcc-api",
          version: "1.0.0-beta.1",
          description: "API",
        },
      },
    }),
  );
}
