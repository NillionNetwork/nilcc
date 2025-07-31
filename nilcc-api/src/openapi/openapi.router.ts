import { cors } from "hono/cors";
import { openAPISpecs } from "hono-openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";

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
          description: `This API lets users create and manipulate workloads in nilcc.

All endpoints require the \`x-api-key\` header set to a valid API key.
`,
        },
        servers: [
          {
            url: "https://nilcc-api.sandbox.app-cluster.sandbox.nilogy.xyz",
            description: "The sandbox environment",
          },
        ],
        components: {
          securitySchemes: {
            ApiKeyAuth: {
              type: "apiKey",
              in: "header",
              name: "x-api-key",
            },
          },
        },
        security: [{ ApiKeyAuth: [] }],
      },
    }),
  );
}
