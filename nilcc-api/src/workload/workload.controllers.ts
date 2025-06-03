import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import z from "zod";
import { errorHandler } from "#/common/handler";
import {
  OpenApiSpecCommonErrorResponses,
  OpenApiSpecEmptySuccessResponses,
} from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { paramsValidator, validateResponse } from "#/common/zod-utils";
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
  const path = PathsV1.workload.create;

  app.post(
    path,
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
    errorHandler(),
    async (c) => {
      const payload = c.req.valid("json");
      const workload = await workloadService.create(bindings, payload);
      return validateResponse(
        CreateWorkloadResponse,
        workloadMapper.entityToResponse(workload),
        c,
      );
    },
  );
}

export function list(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.list;

  app.get(
    path,
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
    errorHandler(),
    async (c) => {
      const workloads = await workloadService.list(bindings);
      return validateResponse(
        CreateWorkloadResponse.array(),
        workloads.map(workloadMapper.entityToResponse),
        c,
      );
    },
  );
}

export function read(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.read;

  app.get(
    path,
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
    errorHandler(),
    paramsValidator(idParamSchema),
    async (c) => {
      const params = c.req.valid("param");
      const workload = await workloadService.read(bindings, params.id);
      if (!workload) {
        return c.notFound();
      }
      return validateResponse(
        CreateWorkloadResponse,
        workloadMapper.entityToResponse(workload),
        c,
      );
    },
  );
}

export function update(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.update;

  app.put(
    path,
    describeRoute({
      tags: ["Workload"],
      summary: "Update a Workload",
      description: "Update a Workload",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    errorHandler(),
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
  const path = PathsV1.workload.remove;

  app.delete(
    path,
    describeRoute({
      tags: ["Workload"],
      summary: "Remove a Workload",
      description: "Remove a Workload",
      responses: {
        200: OpenApiSpecEmptySuccessResponses[200],
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    errorHandler(),
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
