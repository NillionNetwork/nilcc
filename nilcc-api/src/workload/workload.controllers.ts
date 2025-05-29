import { Effect as E, pipe } from "effect";
import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import z from "zod";
import { handleTaggedErrors } from "#/common/handler";
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
import * as WorkloadService from "./workload.service";

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
    async (c) => {
      return pipe(
        c.req.valid("json"),
        (payload) => WorkloadService.create(bindings, payload),
        E.flatMap((workload) =>
          validateResponse(
            CreateWorkloadResponse,
            workloadMapper.entityToResponse(workload),
            c,
          ),
        ),
        handleTaggedErrors(c),
        E.runPromise,
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
    async (c) => {
      return pipe(
        WorkloadService.list(bindings),
        E.flatMap((workloads) =>
          validateResponse(
            CreateWorkloadResponse.array(),
            workloads.map(workloadMapper.entityToResponse),
            c,
          ),
        ),
        handleTaggedErrors(c),
        E.runPromise,
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
    paramsValidator(idParamSchema),
    async (c) => {
      return pipe(
        c.req.valid("param"),
        (params) => WorkloadService.read(bindings, params.id),
        E.flatMap((workload) => {
          if (!workload) {
            return E.succeed(c.notFound()) as E.Effect<Response, never, never>;
          }
          return validateResponse(
            CreateWorkloadResponse,
            workloadMapper.entityToResponse(workload),
            c,
          );
        }),
        handleTaggedErrors(c),
        E.runPromise,
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
    zValidator("json", UpdateWorkloadRequest),
    async (c) => {
      return pipe(
        c.req.valid("json"),
        (payload) => WorkloadService.update(bindings, payload),
        E.flatMap((updated) => {
          if (!updated) {
            return E.succeed(c.notFound()) as E.Effect<Response, never, never>;
          }
          return E.succeed(c.body(null, 200));
        }),
        handleTaggedErrors(c),
        E.runPromise,
      );
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
    paramsValidator(idParamSchema),
    async (c) => {
      return pipe(
        c.req.valid("param").id,
        (workloadId) => WorkloadService.remove(bindings, workloadId),
        E.flatMap((deleted) => {
          if (!deleted) {
            return E.succeed(c.notFound()) as E.Effect<Response, never, never>;
          }
          return E.succeed(c.body(null, 200));
        }),
        handleTaggedErrors(c),
        E.runPromise,
      );
    },
  );
}
