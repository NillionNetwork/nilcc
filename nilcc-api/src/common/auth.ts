import type { Context, Next } from "hono";
import { Temporal } from "temporal-polyfill";
import type { AppBindings } from "#/env";
import type { ApiErrorResponse } from "./handler";

export function adminOrUserAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.adminApiKey);
}

export function adminAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.adminApiKey);
}

export function metalInstanceAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.metalInstanceApiKey);
}

export function userAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const apiToken = c.req.header("x-api-key");
    if (!apiToken) {
      return c.json(authError("no x-api-key header provided"), 401);
    }
    const account = await bindings.services.account.findByApiToken(
      bindings,
      apiToken,
    );
    if (account === null) {
      return c.json(authError("unauthorized"), 401);
    }
    c.set("account", account);
    await next();
    return;
  };
}

function authError(error: string): ApiErrorResponse {
  return {
    ts: Temporal.Now.instant().toString(),
    error,
    kind: "UNAUTHORIZED",
  };
}

function requireApiKey(apiKey: string) {
  return async (c: Context, next: Next) => {
    const requestApiKey = c.req.header("x-api-key");
    if (!requestApiKey || requestApiKey !== apiKey) {
      return c.json(authError("unauthorized"), 401);
    }
    await next();
    return;
  };
}
