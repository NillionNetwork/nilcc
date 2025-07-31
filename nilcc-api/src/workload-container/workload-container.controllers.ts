import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { apiKey } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator, responseValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  ListContainersRequest,
  ListContainersResponse,
  WorkloadContainerLogsRequest,
  WorkloadContainerLogsResponse,
} from "./workload-container.dto";

export function listContainers(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadContainers.list,
    describeRoute({
      tags: ["workload", "container"],
      summary: "List the containers for a workload",
      description:
        "This endpoint retrieves some basic information about the container being ran by a workload.",
      responses: {
        200: {
          description: "The containers list is returned",
          content: {
            "application/json": {
              schema: resolver(ListContainersResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    payloadValidator(ListContainersRequest),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, ListContainersResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const containers = await bindings.services.workload.listContainers(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(containers);
    },
  );
}

export function containerLogs(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadContainers.logs,
    describeRoute({
      tags: ["workload", "container"],
      summary: "Get the logs for a workload's container",
      description:
        "This endpoint retrieves the logs for a container running in a workload",
      responses: {
        200: {
          description: "The container logs",
          content: {
            "application/json": {
              schema: resolver(WorkloadContainerLogsRequest),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    payloadValidator(WorkloadContainerLogsRequest),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, WorkloadContainerLogsResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const logs = await bindings.services.workload.containerLogs(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(logs);
    },
  );
}
