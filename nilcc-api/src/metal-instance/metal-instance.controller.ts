import { describeRoute } from "hono-openapi";
import { resolver, validator as zValidator } from "hono-openapi/zod";
import z from "zod";
import { apiKey } from "#/common/auth";
import { errorHandler } from "#/common/handler";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { paramsValidator, responseValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  GetMetalInstanceResponse,
  ListMetalInstancesResponse,
  RegisterMetalInstanceRequest,
} from "#/metal-instance/metal-instance.dto";
import { metalInstanceMapper } from "#/metal-instance/metal-instance.mapper";
import { metalInstanceService } from "#/metal-instance/metal-instance.service";

const idParamSchema = z.object({ id: z.string().uuid() });

export function register(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.register,
    describeRoute({
      tags: ["Metal-Instance"],
      summary:
        "Register a Metal Instance, will create it if it does not exist, or update it if it does",
      description:
        "Registers a Metal Instance by creating it if it does not exist, or updating it if it does",
      responses: {
        200: {
          description: "Metal Instance registered successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    zValidator("json", RegisterMetalInstanceRequest),
    errorHandler(),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      await metalInstanceService.createOrUpdate(bindings, payload);
      return c.body(null);
    },
  );
}

export function sync(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.metalInstance.sync,
    describeRoute({
      tags: ["Metal-Instance"],
      summary:
        "Sync a Metal Instance State receiving current state and answering with desired state",
      description:
        "Sync a Metal Instance State receiving current state and answering with desired state",
      responses: {
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    apiKey(bindings.config.metalInstanceApiKey),
    errorHandler(),
    async (c) => {
      // Placeholder for creating a metal instance
      return c.json({ message: "Metal instance created" });
    },
  );
}

export function read(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.metalInstance.read,
    describeRoute({
      tags: ["Metal-Instance"],
      summary: "Get a Metal Instance by ID",
      description: "Returns a Metal Instance by its ID",
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
    paramsValidator(idParamSchema),
    errorHandler(),
    responseValidator(bindings, GetMetalInstanceResponse),
    async (c) => {
      const id = c.req.param("id");
      const instance = await metalInstanceService.read(bindings, id);
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
      summary: "List all Metal Instances",
      description: "Returns a list of all Metal Instances",
      responses: {
        200: {
          description: "List of Metal Instances",
          content: {
            "application/json": {
              schema: resolver(ListMetalInstancesResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    errorHandler(),
    responseValidator(bindings, ListMetalInstancesResponse),
    async (c) => {
      const instances = await metalInstanceService.list(bindings);
      return c.json(instances);
    },
  );
}
