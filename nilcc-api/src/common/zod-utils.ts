import { zValidator } from "@hono/zod-validator";
import type { Context, Next } from "hono";
import { StatusCodes } from "http-status-codes";
import { Temporal } from "temporal-polyfill";
import type { Schema } from "zod";
import type { AppBindings } from "#/env";
import type { ApiErrorResponse } from "./handler";

export function payloadValidator<T extends Schema>(schema: T) {
  return zValidator("json", schema, (result, c) => {
    if (result.success) {
      return result.data;
    }

    const response: ApiErrorResponse = {
      kind: "INVALID_PAYLOAD",
      error: result.error,
      ts: Temporal.Now.instant().toString(),
    };
    return c.json(response, StatusCodes.BAD_REQUEST);
  });
}

export function pathValidator<T extends Schema>(schema: T) {
  return zValidator("param", schema, (result, c) => {
    if (result.success) {
      return result.data;
    }

    const response: ApiErrorResponse = {
      kind: "INVALID_PATH",
      error: result.error,
      ts: Temporal.Now.instant().toString(),
    };
    return c.json(response, StatusCodes.BAD_REQUEST);
  });
}

export function responseValidator<T extends Schema>(
  bindings: AppBindings,
  schema: T,
) {
  return async (c: Context, next: Next) => {
    await next();
    if (!bindings.config.enabledFeatures.includes("response-validation")) {
      return c;
    }
    if (c.res.status < 200 || c.res.status >= 300) {
      return c;
    }
    const result = schema.safeParse(await c.res.clone().json());
    if (result.success) {
      return c;
    }
    const response: ApiErrorResponse = {
      kind: "INVALID_RESPONSE",
      error: result.error,
      ts: Temporal.Now.instant().toString(),
    };

    return c.json(response, StatusCodes.INTERNAL_SERVER_ERROR);
  };
}
