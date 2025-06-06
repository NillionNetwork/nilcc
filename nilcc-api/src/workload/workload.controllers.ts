import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import { StatusCodes } from "http-status-codes";
import z from "zod";
import {
  CreateEntityError,
  HttpError,
  InstancesNotAvailable,
} from "#/common/errors";
import {
  OpenApiSpecCommonErrorResponses,
  OpenApiSpecEmptySuccessResponses,
} from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { paramsValidator, responseValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import { workloadMapper } from "#/workload/workload.mapper";
import {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
  GetWorkloadResponse,
  ListWorkloadsResponse,
  UpdateWorkloadRequest,
} from "./workload.dto";
import { workloadService } from "./workload.service";

const idParamSchema = z.object({ id: z.string().uuid() });

export function create(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.post(
    PathsV1.workload.create,
    describeRoute({
      tags: ["Workload"],
      summary: "Create a new Workload",
      description:
        "Creates a new Workload ad responds with the workload data and Id",
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
    zValidator("json", CreateWorkloadRequest),
    responseValidator(bindings, CreateWorkloadResponse),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      try {
        const workload = await workloadService.create(
          bindings,
          payload,
          c.get("txQueryRunner"),
        );
        return c.json(workloadMapper.entityToResponse(workload));
      } catch (e: unknown) {
        if (
          e instanceof CreateEntityError &&
          e.cause instanceof InstancesNotAvailable
        ) {
          throw new HttpError({
            message: "No available instances to create workload",
            statusCode: StatusCodes.SERVICE_UNAVAILABLE,
          });
        }
        throw e;
      }
    },
  );
}

export function list(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.get(
    PathsV1.workload.list,
    describeRoute({
      tags: ["Workload"],
      summary: "List Workloads",
      description: "List all Workloads",
      responses: {
        200: {
          description: "Workload listed successfully",
          content: {
            "application/json": {
              schema: resolver(ListWorkloadsResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    responseValidator(bindings, CreateWorkloadResponse.array()),
    async (c) => {
      const workloads = await workloadService.list(bindings);
      return c.json(workloads.map(workloadMapper.entityToResponse));
    },
  );
}

export function read(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.get(
    PathsV1.workload.read,
    describeRoute({
      tags: ["Workload"],
      summary: "Read a Workload",
      description: "Read a Workload by its ID",
      responses: {
        200: {
          description: "Workload read successfully",
          content: {
            "application/json": {
              schema: resolver(GetWorkloadResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    paramsValidator(idParamSchema),
    responseValidator(bindings, GetWorkloadResponse),
    async (c) => {
      const params = c.req.valid("param");
      const workload = await workloadService.read(bindings, params.id);
      if (!workload) {
        return c.notFound();
      }
      return c.json(workloadMapper.entityToResponse(workload));
    },
  );
}

export function update(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.put(
    PathsV1.workload.update,
    describeRoute({
      tags: ["Workload"],
      summary: "Update a Workload",
      description: "Update a Workload",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    zValidator("json", UpdateWorkloadRequest),
    async (c) => {
      const payload = c.req.valid("json");
      const updated = await workloadService.update(bindings, payload);
      if (!updated) {
        return c.notFound();
      }
      return c.body(null, 200);
    },
  );
}

export function remove(options: ControllerOptions): void {
  const { app, bindings } = options;

  app.delete(
    PathsV1.workload.remove,
    describeRoute({
      tags: ["Workload"],
      summary: "Remove a Workload",
      description: "Remove a Workload",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    paramsValidator(idParamSchema),
    async (c) => {
      const workloadId = c.req.valid("param").id;
      const deleted = await workloadService.remove(bindings, workloadId);
      if (!deleted) {
        return c.notFound();
      }
      return c.body(null, 200);
    },
  );
}
