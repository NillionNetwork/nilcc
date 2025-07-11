import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import z from "zod";
import { apiKey } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { paramsValidator, responseValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  GetMetalInstanceResponse,
  HeartbeatRequest,
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
      tags: ["Metal-Instance"],
      summary:
        "Register a metal instance, will create it if it does not exist, or update it if it does",
      description:
        "Registers a metal instance by creating it if it does not exist, or updating it if it does",
      responses: {
        200: {
          description: "Metal instance registered successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    zValidator("json", RegisterMetalInstanceRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.metalInstance.createOrUpdate(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.body(null);
    },
  );
}

export function heartbeat(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.heartbeat,
    describeRoute({
      tags: ["Metal-Instance"],
      summary: "Report this metal instance as being online",
      responses: {
        200: {
          description: "The heartbeat was processed successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    zValidator("json", HeartbeatRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.metalInstance.heartbeat(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.body(null);
    },
  );
}

export function read(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.metalInstance.read,
    describeRoute({
      tags: ["Metal-Instance"],
      summary: "Get a metal instance by ID",
      description: "Returns a metal instance by its ID",
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
    apiKey(bindings.config.userApiKey),
    paramsValidator(idParamSchema),
    responseValidator(bindings, GetMetalInstanceResponse),
    async (c) => {
      const id = c.req.param("id");
      const instance = await bindings.services.metalInstance.read(bindings, id);
      if (!instance) {
        return c.notFound();
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
      tags: ["Metal-Instance"],
      summary: "List all metal instances",
      description: "Returns a list of all metal instances",
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
    apiKey(bindings.config.userApiKey),
    responseValidator(bindings, ListMetalInstancesResponse),
    async (c) => {
      const instances = await bindings.services.metalInstance.list(bindings);
      return c.json(instances);
    },
  );
}
