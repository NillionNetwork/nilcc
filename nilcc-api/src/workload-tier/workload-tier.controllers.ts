import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { adminAuthentication, adminOrUserAuthentication } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  CreateWorkloadTierRequest,
  DeleteWorkloadTierRequest,
  UpdateWorkloadTierRequest,
  WorkloadTier,
} from "./workload-tier.dto";
import { workloadTierMapper } from "./workload-tier.mapper";

export function create(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadTiers.create,
    describeRoute({
      tags: ["workload-tier"],
      summary: "Create a workload tier",
      description: "This creates a workload tier.",
      responses: {
        200: {
          description: "The workload tier was created successfully",
          content: {
            "application/json": {
              schema: resolver(WorkloadTier),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(CreateWorkloadTierRequest),
    async (c) => {
      const payload = c.req.valid("json");
      const tier = await bindings.services.workloadTier.create(
        bindings,
        payload,
      );
      return c.json(workloadTierMapper.entityToResponse(tier));
    },
  );
}

export function list(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.workloadTiers.list,
    describeRoute({
      tags: ["workload-tier"],
      summary: "List workload tiers.",
      description: "This endpoint lists all existing workload tiers.",
      responses: {
        200: {
          description: "The workload tiers.",
          content: {
            "application/json": {
              schema: resolver(WorkloadTier.array()),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminOrUserAuthentication(bindings),
    async (c) => {
      const tiers = await bindings.services.workloadTier.list(bindings);
      tiers.sort((a, b) => a.cost - b.cost);
      return c.json(tiers.map(workloadTierMapper.entityToResponse));
    },
  );
}

export function update(options: ControllerOptions) {
  const { app, bindings } = options;
  app.put(
    PathsV1.workloadTiers.update,
    describeRoute({
      tags: ["workload-tier"],
      summary: "Update a workload tier",
      description: "This updates a workload tier.",
      responses: {
        200: {
          description: "The workload tier was updated successfully",
          content: {
            "application/json": {
              schema: resolver(WorkloadTier),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(UpdateWorkloadTierRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const tier = await bindings.services.workloadTier.update(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(workloadTierMapper.entityToResponse(tier));
    },
  );
}

export function remove(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadTiers.delete,
    describeRoute({
      tags: ["workload-tier"],
      summary: "Delete a workload tier.",
      description: "This endpoint deletes a tier.",
      responses: {
        200: {
          description: "The tier was deleted.",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(DeleteWorkloadTierRequest),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.workloadTier.remove(bindings, payload.tierId);
      return c.json({});
    },
  );
}
