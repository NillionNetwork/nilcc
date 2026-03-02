import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { z } from "zod";
import {
  accountIdentityAdminAuthentication,
  assertCanManageIdentityAccount,
} from "#/common/auth";
import { EntityNotFound } from "#/common/errors";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { pathValidator, payloadValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  ApiKey,
  CreateApiKeyRequest,
  DeleteApiKeyRequest,
  ListApiKeysResponse,
  UpdateApiKeyRequest,
} from "./api-key.dto";
import { apiKeyMapper } from "./api-key.mapper";

const accountIdSchema = z.object({ accountId: z.string().uuid() });

export function create(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.apiKeys.create,
    describeRoute({
      tags: ["api-keys"],
      summary: "Create an API key",
      responses: {
        200: {
          description: "API key created successfully",
          content: {
            "application/json": {
              schema: resolver(ApiKey),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    payloadValidator(CreateApiKeyRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      assertCanManageIdentityAccount(c, payload.accountId);
      const apiKey = await bindings.services.apiKey.create(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(apiKeyMapper.entityToResponse(apiKey));
    },
  );
}

export function listByAccount(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.apiKeys.listByAccount,
    describeRoute({
      tags: ["api-keys"],
      summary: "List API keys for an account",
      responses: {
        200: {
          description: "API keys listed successfully",
          content: {
            "application/json": {
              schema: resolver(ListApiKeysResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    pathValidator(accountIdSchema),
    async (c) => {
      const params = c.req.valid("param");
      assertCanManageIdentityAccount(c, params.accountId);
      const keys = await bindings.services.apiKey.listByAccount(
        bindings,
        params.accountId,
      );
      return c.json(keys.map(apiKeyMapper.entityToResponse));
    },
  );
}

export function update(options: ControllerOptions) {
  const { app, bindings } = options;
  app.put(
    PathsV1.apiKeys.update,
    describeRoute({
      tags: ["api-keys"],
      summary: "Update an API key",
      responses: {
        200: {
          description: "API key updated successfully",
          content: {
            "application/json": {
              schema: resolver(ApiKey),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    payloadValidator(UpdateApiKeyRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const existing = await bindings.services.apiKey.read(
        bindings,
        payload.id,
        c.get("txQueryRunner"),
      );
      if (!existing) {
        throw new EntityNotFound("api key");
      }
      assertCanManageIdentityAccount(c, existing.accountId);
      const updated = await bindings.services.apiKey.update(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json(apiKeyMapper.entityToResponse(updated));
    },
  );
}

export function remove(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.apiKeys.delete,
    describeRoute({
      tags: ["api-keys"],
      summary: "Delete an API key",
      responses: {
        200: {
          description: "API key deleted successfully",
          content: {
            "application/json": {
              schema: resolver(z.object({})),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    payloadValidator(DeleteApiKeyRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      const existing = await bindings.services.apiKey.read(
        bindings,
        payload.id,
        c.get("txQueryRunner"),
      );
      if (!existing) {
        throw new EntityNotFound("api key");
      }
      assertCanManageIdentityAccount(c, existing.accountId);
      await bindings.services.apiKey.delete(
        bindings,
        payload.id,
        c.get("txQueryRunner"),
      );
      return c.json({});
    },
  );
}
