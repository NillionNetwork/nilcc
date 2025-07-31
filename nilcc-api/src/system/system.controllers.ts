import { describeRoute } from "hono-openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";

export function health(options: ControllerOptions): void {
  const { app } = options;

  app.get(
    PathsV1.system.health,
    describeRoute({
      tags: ["System"],
      summary: "Health check",
      responses: {
        200: {
          description: "OK",
          content: {
            "text/plain": {
              schema: {
                type: "string",
              },
            },
          },
        },
      },
    }),
    async (c) => c.text("OK"),
  );
}
