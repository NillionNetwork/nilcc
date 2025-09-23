import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import z from "zod";
import {
  adminAuthentication,
  metalInstanceAuthentication,
} from "#/common/auth";
import { EntityNotFound } from "#/common/errors";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import {
  pathValidator,
  payloadValidator,
  responseValidator,
} from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  DeleteMetalInstanceRequest,
  GetMetalInstanceResponse,
  HeartbeatRequest,
  HeartbeatResponse,
  ListMetalInstancesResponse,
  RegisterMetalInstanceRequest,
} from "#/metal-instance/metal-instance.dto";
import { metalInstanceMapper } from "#/metal-instance/metal-instance.mapper";

const idParamSchema = z.object({ id: z.string().uuid() });

export function register(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.register,
    describeRoute({
      tags: ["metal-instance"],
      summary: "Register a metal instance",
      description:
        "This will create the metal instance if it's the first time it's registering, or it will update it if it already exists",
      responses: {
        200: {
          description: "Metal instance registered successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    metalInstanceAuthentication(bindings),
    payloadValidator(RegisterMetalInstanceRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.metalInstance.createOrUpdate(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json({});
    },
  );
}

export function heartbeat(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.heartbeat,
    describeRoute({
      tags: ["metal-instance"],
      summary: "Report a metal instance as being online",
      description:
        "This endpoint must be called periodically by every metal instance for them to be considered online by nilcc-api",
      responses: {
        200: {
          description: "The heartbeat was processed successfully",
          content: {
            "application/json": {
              schema: resolver(HeartbeatResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    metalInstanceAuthentication(bindings),
    payloadValidator(HeartbeatRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const response = await bindings.services.metalInstance.heartbeat(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(response);
    },
  );
}

export function read(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.metalInstance.read,
    describeRoute({
      tags: ["metal-instance"],
      summary: "Get the details for a metal instance by ID",
      responses: {
        200: {
          description: "Workload found",
          content: {
            "application/json": {
              schema: resolver(GetMetalInstanceResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    pathValidator(idParamSchema),
    responseValidator(bindings, GetMetalInstanceResponse),
    async (c) => {
      const id = c.req.param("id");
      const instance = await bindings.services.metalInstance.read(bindings, id);
      if (!instance) {
        throw new EntityNotFound("metal instance");
      }
      return c.json(metalInstanceMapper.entityToResponse(instance));
    },
  );
}

export function list(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.metalInstance.list,
    describeRoute({
      tags: ["metal-instance"],
      summary: "List all metal instances",
      responses: {
        200: {
          description: "List of metal instances",
          content: {
            "application/json": {
              schema: resolver(ListMetalInstancesResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    responseValidator(bindings, ListMetalInstancesResponse),
    async (c) => {
      const instances = await bindings.services.metalInstance.list(bindings);
      return c.json(instances);
    },
  );
}

export function remove(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.delete,
    describeRoute({
      tags: ["metal-instance"],
      summary: "Delete a metal instance",
      responses: {
        200: {
          description: "The metal instance was deleted",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(DeleteMetalInstanceRequest),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.metalInstance.remove(
        bindings,
        payload.metalInstanceId,
      );
      return c.json({});
    },
  );
}
