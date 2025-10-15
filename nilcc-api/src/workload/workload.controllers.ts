import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import z from "zod";
import { SystemStatsResponse as StatsResponse } from "#/clients/nilcc-agent.client";
import { userAuthentication } from "#/common/auth";
import { EntityNotFound } from "#/common/errors";
import {
  OpenApiSpecCommonErrorResponses,
  OpenApiSpecEmptySuccessResponses,
} from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import {
  pathValidator,
  payloadValidator,
  responseValidator,
} from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import { workloadMapper } from "#/workload/workload.mapper";
import {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
  DeleteWorkloadRequest,
  GetWorkloadResponse,
  ListWorkloadsResponse,
  RestartWorkloadRequest,
  StatsRequest,
  WorkloadSystemLogsRequest,
  WorkloadSystemLogsResponse,
} from "./workload.dto";

const idParamSchema = z.object({ id: z.string().uuid() });

export function create(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.post(
    PathsV1.workload.create,
    describeRoute({
      tags: ["workload"],
      summary: "Create a new workload",
      description:
        "This endpoint creates a new workload. The domain the workload is accessible at will be returned as part of the response.",
      responses: {
        200: {
          description: "Workload created successfully",
          content: {
            "application/json": {
              schema: resolver(CreateWorkloadResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(CreateWorkloadRequest),
    responseValidator(bindings, CreateWorkloadResponse),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const workload = await bindings.services.workload.create(
        bindings,
        payload,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json(
        workloadMapper.entityToResponse(
          workload,
          bindings.config.workloadsDnsDomain,
          bindings.config.metalInstancesDnsDomain,
        ),
      );
    },
  );
}

export function list(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.get(
    PathsV1.workload.list,
    describeRoute({
      tags: ["workload"],
      summary: "List all workloads",
      responses: {
        200: {
          description: "The workloads",
          content: {
            "application/json": {
              schema: resolver(ListWorkloadsResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    responseValidator(bindings, CreateWorkloadResponse.array()),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const workloads = await bindings.services.workload.list(
        bindings,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json(
        workloads.map((w) =>
          workloadMapper.entityToResponse(
            w,
            bindings.config.workloadsDnsDomain,
            bindings.config.metalInstancesDnsDomain,
          ),
        ),
      );
    },
  );
}

export function read(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.get(
    PathsV1.workload.read,
    describeRoute({
      tags: ["workload"],
      summary: "Get the details for a workload",
      description:
        "This endpoint allows getting the details for a workload by its id",
      responses: {
        200: {
          description: "The workload details",
          content: {
            "application/json": {
              schema: resolver(GetWorkloadResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    pathValidator(idParamSchema),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, GetWorkloadResponse),
    async (c) => {
      const params = c.req.valid("param");
      const workload = await bindings.services.workload.read(
        bindings,
        params.id,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      if (!workload) {
        throw new EntityNotFound("workload");
      }
      return c.json(
        workloadMapper.entityToResponse(
          workload,
          bindings.config.workloadsDnsDomain,
          bindings.config.metalInstancesDnsDomain,
        ),
      );
    },
  );
}

export function remove(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.post(
    PathsV1.workload.delete,
    describeRoute({
      tags: ["workload"],
      summary: "Delete a workload",
      description:
        "This endpoint deletes a workload. The workload CVM will be stopped and all resources associated with it will be deleted",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(DeleteWorkloadRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const workloadId = c.req.valid("json").workloadId;
      await bindings.services.workload.remove(
        bindings,
        workloadId,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json({});
    },
  );
}

export function restart(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.post(
    PathsV1.workload.restart,
    describeRoute({
      tags: ["workload"],
      summary: "Restart a workload",
      description: "This endpoint restarts a workload.",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(RestartWorkloadRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const workloadId = c.req.valid("json").workloadId;
      await bindings.services.workload.restart(
        bindings,
        workloadId,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json({});
    },
  );
}

export function systemLogs(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workload.logs,
    describeRoute({
      tags: ["workload"],
      summary: "Get the system logs for a workload",
      description: "This endpoint retrieves the system logs for a workload",
      responses: {
        200: {
          description: "The system logs",
          content: {
            "application/json": {
              schema: resolver(WorkloadSystemLogsResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(WorkloadSystemLogsRequest),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, WorkloadSystemLogsResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const lines = await bindings.services.workload.systemLogs(
        bindings,
        payload,
        c.get("account"),
        c.get("txQueryRunner"),
      );

      const response: WorkloadSystemLogsResponse = { lines };
      return c.json(response);
    },
  );
}

export function stats(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workload.stats,
    describeRoute({
      tags: ["workload"],
      summary: "Get the system stats for a workload",
      description:
        "This endpoint retrieves the system stats (CPU, memory, disk, etc) for a workload.",
      responses: {
        200: {
          description: "The system stats",
          content: {
            "application/json": {
              schema: resolver(StatsResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(StatsRequest),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, StatsResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const response = await bindings.services.workload.systemStats(
        bindings,
        payload,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json(response);
    },
  );
}
