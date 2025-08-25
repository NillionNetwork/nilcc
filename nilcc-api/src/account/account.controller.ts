import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { z } from "zod";
import { adminAuthentication } from "#/common/auth";
import { EntityNotFound } from "#/common/errors";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { pathValidator, payloadValidator } from "#/common/zod-utils";
import { Account, CreateAccountRequest } from "./account.dto";
import { accountMapper } from "./account.mapper";

const idParamSchema = z.object({ id: z.string().uuid() });

export function create(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.account.create,
    describeRoute({
      tags: ["account"],
      summary: "Create a new account",
      description:
        "This will create an account and return its details including its API key",
      responses: {
        200: {
          description: "Account created successfully",
          content: {
            "application/json": {
              schema: resolver(Account),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    payloadValidator(CreateAccountRequest),
    async (c) => {
      const payload = c.req.valid("json");
      const account = await bindings.services.account.create(
        bindings,
        payload.name,
      );
      return c.json(accountMapper.entityToResponse(account));
    },
  );
}

export function list(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.account.list,
    describeRoute({
      tags: ["account"],
      summary: "List all accounts",
      description: "This endpoint lists all available accounts",
      responses: {
        200: {
          description: "The accounts were listed successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    async (c) => {
      const accounts = await bindings.services.account.list(bindings);
      return c.json(accounts.map(accountMapper.entityToResponse));
    },
  );
}

export function read(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.account.read,
    describeRoute({
      tags: ["account"],
      summary: "Get information about an account",
      description:
        "This endpoint fetches all information about an account by id",
      responses: {
        200: {
          description: "The account information was retrieved successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminAuthentication(bindings),
    pathValidator(idParamSchema),
    async (c) => {
      const params = c.req.valid("param");
      const account = await bindings.services.account.read(bindings, params.id);
      if (!account) {
        throw new EntityNotFound("account");
      }
      return c.json(accountMapper.entityToResponse(account));
    },
  );
}
