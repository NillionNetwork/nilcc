import type { Context, Next } from "hono";
import { Temporal } from "temporal-polyfill";
import type { AccountEntity } from "#/account/account.entity";
import type { ApiKeyType } from "#/api-key/api-key.dto";
import { AccessDenied } from "#/common/errors";
import type { AppBindings } from "#/env";
import type { ApiErrorResponse } from "./handler";

export enum AuthPrincipal {
  GLOBAL_ADMIN = "global-admin",
  ACCOUNT_JWT = "account-jwt",
  ACCOUNT_API_KEY = "account-api-key",
}

export type AuthContext =
  | { principal: AuthPrincipal.GLOBAL_ADMIN }
  | { principal: AuthPrincipal.ACCOUNT_JWT }
  | {
      principal: AuthPrincipal.ACCOUNT_API_KEY;
      apiKeyType: ApiKeyType;
    };

export function adminOrUserAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const auth = await resolveAuthentication(c, bindings);
    if (!auth) {
      return c.json(authError("unauthorized"), 401);
    }

    if (auth.principal === AuthPrincipal.GLOBAL_ADMIN) {
      c.set("auth", auth);
      await next();
      return;
    }

    c.set("auth", auth);
    c.set("account", auth.account);
    await next();
    return;
  };
}

export function adminAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const auth = await resolveAuthentication(c, bindings);
    if (!auth || auth.principal !== AuthPrincipal.GLOBAL_ADMIN) {
      return c.json(authError("unauthorized"), 401);
    }

    c.set("auth", auth);
    await next();
    return;
  };
}

export function metalInstanceAuthentication(bindings: AppBindings) {
  return requireApiKey(bindings.config.metalInstanceApiKey);
}

export function userAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const auth = await resolveAuthentication(c, bindings);
    if (!auth || auth.principal === AuthPrincipal.GLOBAL_ADMIN) {
      return c.json(authError("unauthorized"), 401);
    }

    c.set("auth", auth);
    c.set("account", auth.account);
    await next();
    return;
  };
}

export function accountIdentityAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const auth = await resolveAuthentication(c, bindings);
    if (!auth || auth.principal === AuthPrincipal.GLOBAL_ADMIN) {
      return c.json(authError("unauthorized"), 401);
    }

    if (
      auth.principal === AuthPrincipal.ACCOUNT_API_KEY &&
      auth.apiKeyType !== "account-admin"
    ) {
      return c.json(authError("unauthorized"), 401);
    }

    c.set("auth", auth);
    c.set("account", auth.account);
    await next();
    return;
  };
}

export function jwtAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const token = extractBearerToken(c);
    if (!token) {
      return c.json(authError("unauthorized"), 401);
    }

    const auth = await resolveJwtAuthentication(bindings, token);
    if (!auth) {
      return c.json(authError("unauthorized"), 401);
    }

    c.set("auth", auth);
    c.set("account", auth.account);
    await next();
    return;
  };
}

export function accountIdentityAdminAuthentication(bindings: AppBindings) {
  return async (c: Context, next: Next) => {
    const auth = await resolveAuthentication(c, bindings);
    if (!auth) {
      return c.json(authError("unauthorized"), 401);
    }

    if (
      auth.principal === AuthPrincipal.GLOBAL_ADMIN ||
      auth.principal === AuthPrincipal.ACCOUNT_JWT
    ) {
      c.set("auth", auth);
      if ("account" in auth) {
        c.set("account", auth.account);
      }
      await next();
      return;
    }

    if (auth.apiKeyType !== "account-admin") {
      return c.json(authError("unauthorized"), 401);
    }

    c.set("auth", auth);
    c.set("account", auth.account);
    await next();
    return;
  };
}

export function assertCanManageIdentityAccount(
  c: Context,
  targetAccountId: string,
): void {
  const auth = c.get("auth") as AuthContext;
  if (auth.principal === AuthPrincipal.GLOBAL_ADMIN) {
    return;
  }

  const account = c.get("account");
  if (!account || account.id !== targetAccountId) {
    throw new AccessDenied();
  }
}

type ResolvedAuth =
  | { principal: AuthPrincipal.GLOBAL_ADMIN }
  | { principal: AuthPrincipal.ACCOUNT_JWT; account: AccountEntity }
  | {
      principal: AuthPrincipal.ACCOUNT_API_KEY;
      apiKeyType: ApiKeyType;
      account: AccountEntity;
    };

async function resolveAuthentication(
  c: Context,
  bindings: AppBindings,
): Promise<ResolvedAuth | null> {
  const staticAdminApiKey = c.req.header("x-api-key");
  if (staticAdminApiKey && staticAdminApiKey === bindings.config.adminApiKey) {
    return { principal: AuthPrincipal.GLOBAL_ADMIN };
  }

  const token = extractBearerToken(c);
  if (!token) {
    return null;
  }

  const jwtAuth = await resolveJwtAuthentication(bindings, token);
  if (jwtAuth) {
    return jwtAuth;
  }

  const key = await bindings.services.apiKey.findActiveById(bindings, token);
  if (!key) {
    return null;
  }

  const account = await bindings.services.account.read(bindings, key.accountId);
  if (!account) {
    return null;
  }

  return {
    principal: AuthPrincipal.ACCOUNT_API_KEY,
    apiKeyType: key.type,
    account,
  };
}

async function resolveJwtAuthentication(
  bindings: AppBindings,
  token: string,
): Promise<{
  principal: AuthPrincipal.ACCOUNT_JWT;
  account: AccountEntity;
} | null> {
  try {
    const payload = await bindings.services.auth.verifyToken(bindings, token);
    const account = await bindings.services.account.read(bindings, payload.sub);
    if (!account) {
      return null;
    }
    return {
      principal: AuthPrincipal.ACCOUNT_JWT,
      account,
    };
  } catch {
    return null;
  }
}

function extractBearerToken(c: Context): string | null {
  const authHeader = c.req.header("authorization");
  if (authHeader?.startsWith("Bearer ")) {
    return authHeader.slice(7);
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
