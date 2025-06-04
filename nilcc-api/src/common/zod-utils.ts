import { zValidator } from "@hono/zod-validator";
import type { Context, Next } from "hono";
import { StatusCodes } from "http-status-codes";
import { Temporal } from "temporal-polyfill";
import type { Schema } from "zod";
import type { AppBindings } from "#/env";
import { DataValidationError } from "./errors";

export function payloadValidator<T extends Schema>(schema: T) {
  return zValidator("json", schema, (result, c) => {
    if (result.success) {
      return result.data;
    }

    const errors = new DataValidationError({
      issues: [result.error],
      cause: null,
    }).humanize();

    return c.json(
      { ts: Temporal.Now.instant().toString(), errors },
      StatusCodes.BAD_REQUEST,
    );
  });
}

export function paramsValidator<T extends Schema>(schema: T) {
  return zValidator("param", schema, (result, c) => {
    if (result.success) {
      return result.data;
    }

    const errors = new DataValidationError({
      issues: [result.error],
      cause: null,
    }).humanize();

    return c.json(
      { ts: Temporal.Now.instant().toString(), errors },
      StatusCodes.BAD_REQUEST,
    );
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
    const errors = new DataValidationError({
      issues: [result.error],
      cause: null,
    }).humanize();

    return c.json(
      { ts: Temporal.Now.instant().toString(), errors },
      StatusCodes.INTERNAL_SERVER_ERROR,
    );
  };
}
