import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import { StatusCodes } from "http-status-codes";
import z from "zod";
import { apiKey } from "#/common/auth";
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
import { SubmitEventRequest } from "#/metal-instance/metal-instance.dto";
import { workloadMapper } from "#/workload/workload.mapper";
import {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
  GetWorkloadResponse,
  ListWorkloadsResponse,
} from "./workload.dto";

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
    apiKey(bindings.config.userApiKey),
    zValidator("json", CreateWorkloadRequest),
    responseValidator(bindings, CreateWorkloadResponse),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      try {
        const workload = await bindings.services.workload.create(
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
    apiKey(bindings.config.userApiKey),
    responseValidator(bindings, CreateWorkloadResponse.array()),
    async (c) => {
      const workloads = await bindings.services.workload.list(bindings);
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
    apiKey(bindings.config.userApiKey),
    paramsValidator(idParamSchema),
    responseValidator(bindings, GetWorkloadResponse),
    async (c) => {
      const params = c.req.valid("param");
      const workload = await bindings.services.workload.read(
        bindings,
        params.id,
      );
      if (!workload) {
        return c.notFound();
      }
      return c.json(workloadMapper.entityToResponse(workload));
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
    apiKey(bindings.config.userApiKey),
    paramsValidator(idParamSchema),
    async (c) => {
      const workloadId = c.req.valid("param").id;
      const deleted = await bindings.services.workload.remove(
        bindings,
        workloadId,
      );
      if (!deleted) {
        return c.notFound();
      }
      return c.body(null, 200);
    },
  );
}

export function submitEvent(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workload.events.submit,
    describeRoute({
      tags: ["Metal-Instance"],
      summary: "Report an event for a workload running inside a metal instance",
      description: "Reports an event and updates the state for the workload",
      responses: {
        200: {
          description: "The event was processed successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    zValidator("json", SubmitEventRequest),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.workload.submitEvent(bindings, payload);
      return c.body(null);
    },
  );
}
