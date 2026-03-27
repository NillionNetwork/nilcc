import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { z } from "zod";
import {
  accountIdentityAdminAuthentication,
  accountIdentityAuthentication,
  adminAuthentication,
  assertCanManageIdentityAccount,
} from "#/common/auth";
import { EntityNotFound } from "#/common/errors";
import { microdollarsToUsd } from "#/common/nil";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { pathValidator, payloadValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  Account,
  AddBalanceRequest,
  CreateAccountRequest,
  MyAccount,
  UpdateAccountRequest,
} from "./account.dto";
import { accountMapper } from "./account.mapper";

const idParamSchema = z.object({ id: z.string().uuid() });

export function create(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.account.create,
    describeRoute({
      tags: ["account"],
      summary: "Create a new account",
      description: "This will create an account and return its details.",
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
      const account = await bindings.services.account.create(bindings, payload);
      return c.json(accountMapper.entityToResponse(account));
    },
  );
}

export function update(options: ControllerOptions) {
  const { app, bindings } = options;
  app.put(
    PathsV1.account.update,
    describeRoute({
      tags: ["account"],
      summary: "Update an account",
      description: "This updates an account's properties.",
      responses: {
        200: {
          description: "Account updated successfully",
          content: {
            "application/json": {
              schema: resolver(Account),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    payloadValidator(UpdateAccountRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      assertCanManageIdentityAccount(c, payload.accountId);
      const account = await bindings.services.account.update(
        bindings,
        payload,
        c.get("txQueryRunner"),
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
          content: {
            "application/json": {
              schema: resolver(Account),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    pathValidator(idParamSchema),
    async (c) => {
      const params = c.req.valid("param");
      assertCanManageIdentityAccount(c, params.id);
      const account = await bindings.services.account.read(bindings, params.id);
      if (!account) {
        throw new EntityNotFound("account");
      }
      return c.json(accountMapper.entityToResponse(account));
    },
  );
}

export function me(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.account.me,
    describeRoute({
      tags: ["account"],
      summary: "Get information about the user's account.",
      description: "This endpoint returns information about the account.",
      responses: {
        200: {
          description: "The account information was retrieved successfully",
          content: {
            "application/json": {
              schema: resolver(MyAccount),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAuthentication(bindings),
    async (c) => {
      const account = c.get("account");
      const outputAccount = accountMapper.entityToResponse(account);
      const spendingMicrodollars =
        await bindings.services.account.getAccountUsdSpending(
          bindings,
          account.id,
        );
      const burnRatePerMin = microdollarsToUsd(spendingMicrodollars);
      return c.json(MyAccount.parse({ burnRatePerMin, ...outputAccount }));
    },
  );
}

export function addBalance(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.account.addBalance,
    describeRoute({
      tags: ["account"],
      summary: "Add USD balance to an account.",
      description: "This will add USD balance to the given account.",
      responses: {
        200: {
          description: "The balance was added successfully",
          content: {
            "application/json": {
              schema: resolver(Account),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    accountIdentityAdminAuthentication(bindings),
    payloadValidator(AddBalanceRequest),
    async (c) => {
      const payload = c.req.valid("json");
      assertCanManageIdentityAccount(c, payload.accountId);
      const account = await bindings.services.account.addBalance(
        bindings,
        payload,
      );
      return c.json(accountMapper.entityToResponse(account));
    },
  );
}
