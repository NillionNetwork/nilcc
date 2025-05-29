import { Effect as E, pipe } from "effect";
import z from "zod";
import { handleTaggedErrors } from "#/common/handler";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import {
  paramsValidator,
  payloadValidator,
  validateResponseE,
} from "#/common/zod-utils";
import type { WorkloadEntity } from "#/workload/workload.entity";
import {
  ApiRequestCreateWorkloadSchema,
  ApiRequestUpdateWorkloadSchema,
  ApiResponseCreateWorkloadSchema,
} from "./workload.api";
import * as WorkloadService from "./workload.service";

const idParamSchema = z.object({ id: z.string().uuid() });

function mapEntityToResponse(workload: WorkloadEntity) {
  return {
    id: workload.id,
    name: workload.name,
    description: workload.description ? workload.description : undefined,
    tags: workload.tags ? workload.tags : undefined,
    dockerCompose: workload.dockerCompose,
    serviceToExpose: workload.serviceToExpose,
    servicePortToExpose: workload.servicePortToExpose,
    memory: workload.memory,
    cpu: workload.cpu,
    status: workload.status,
    createdAt: workload.createdAt.toISOString(),
    updatedAt: workload.updatedAt.toISOString(),
  };
}

export function create(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.create;

  app.post(
    path,
    payloadValidator(ApiRequestCreateWorkloadSchema),
    async (c) => {
      return pipe(
        c.req.valid("json"),
        (payload) => WorkloadService.create(bindings, payload),
        E.flatMap((workload) =>
          validateResponseE(
            ApiResponseCreateWorkloadSchema,
            mapEntityToResponse(workload),
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

  app.get(path, async (c) => {
    return pipe(
      WorkloadService.list(bindings),
      E.flatMap((workloads) =>
        validateResponseE(
          ApiResponseCreateWorkloadSchema.array(),
          workloads.map(mapEntityToResponse),
          c,
        ),
      ),
      handleTaggedErrors(c),
      E.runPromise,
    );
  });
}

export function read(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.read;

  app.get(path, paramsValidator(idParamSchema), async (c) => {
    return pipe(
      c.req.valid("param"),
      (params) => WorkloadService.read(bindings, params.id),
      E.flatMap((workload) => {
        if (!workload) {
          return E.succeed(c.notFound()) as E.Effect<Response, never, never>;
        }
        return validateResponseE(
          ApiResponseCreateWorkloadSchema,
          mapEntityToResponse(workload),
          c,
        );
      }),
      handleTaggedErrors(c),
      E.runPromise,
    );
  });
}

export function update(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.update;

  app.put(path, payloadValidator(ApiRequestUpdateWorkloadSchema), async (c) => {
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
  });
}

export function remove(options: ControllerOptions): void {
  const { app, bindings } = options;
  const path = PathsV1.workload.remove;

  app.delete(path, paramsValidator(idParamSchema), async (c) => {
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
  });
}
