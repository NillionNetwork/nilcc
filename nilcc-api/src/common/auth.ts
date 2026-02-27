import type { Context, Next } from "hono";
import { Temporal } from "temporal-polyfill";
import type { AppBindings } from "#/env";
import type { ApiErrorResponse } from "./handler";

export function adminOrUserAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    // Try admin API key first
    const apiKey = c.req.header("x-api-key");
    if (apiKey && apiKey === bindings.config.adminApiKey) {
      await next();
      return;
    }

    // Try JWT bearer token
    const account = await resolveAccountFromJwt(c, bindings);
    if (account) {
      c.set("account", account);
      await next();
      return;
    }

    return c.json(authError("unauthorized"), 401);
  };
}

export function adminAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.adminApiKey);
}

export function metalInstanceAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.metalInstanceApiKey);
}

export function userAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const account = await resolveAccountFromJwt(c, bindings);
    if (!account) {
      return c.json(authError("unauthorized"), 401);
    }
    c.set("account", account);
    await next();
    return;
  };
}

async function resolveAccountFromJwt(c: Context, bindings: AppBindings) {
  const token = extractBearerToken(c);
  if (!token) {
    return null;
  }
  try {
    const payload = await bindings.services.auth.verifyToken(bindings, token);
    return await bindings.services.account.read(bindings, payload.sub);
  } catch {
    return null;
  }
}

function extractBearerToken(c: Context): string | null {
  const authHeader = c.req.header("authorization");
  if (authHeader?.startsWith("Bearer ")) {
    return authHeader.slice(7);
  }
  // Fallback: check x-api-key header for JWT tokens
  const apiKey = c.req.header("x-api-key");
  if (apiKey?.includes(".")) {
    return apiKey;
  }
  return null;
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
