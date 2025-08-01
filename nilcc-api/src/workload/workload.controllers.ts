import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import z from "zod";
import { apiKey } from "#/common/auth";
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
    apiKey(bindings.config.userApiKey),
    payloadValidator(CreateWorkloadRequest),
    responseValidator(bindings, CreateWorkloadResponse),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const workload = await bindings.services.workload.create(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(
        workloadMapper.entityToResponse(
          workload,
          bindings.config.workloadsDnsDomain,
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
    apiKey(bindings.config.userApiKey),
    responseValidator(bindings, CreateWorkloadResponse.array()),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const workloads = await bindings.services.workload.list(
        bindings,
        c.get("txQueryRunner"),
      );
      return c.json(
        workloads.map((w) =>
          workloadMapper.entityToResponse(
            w,
            bindings.config.workloadsDnsDomain,
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
    apiKey(bindings.config.userApiKey),
    pathValidator(idParamSchema),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, GetWorkloadResponse),
    async (c) => {
      const params = c.req.valid("param");
      const workload = await bindings.services.workload.read(
        bindings,
        params.id,
        c.get("txQueryRunner"),
      );
      if (!workload) {
        throw new EntityNotFound("workload");
      }
      return c.json(
        workloadMapper.entityToResponse(
          workload,
          bindings.config.workloadsDnsDomain,
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
    apiKey(bindings.config.userApiKey),
    payloadValidator(DeleteWorkloadRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const workloadId = c.req.valid("json").id;
      await bindings.services.workload.remove(
        bindings,
        workloadId,
        c.get("txQueryRunner"),
      );
      return c.body(null);
    },
  );
}
