import type { Context } from "hono";
import type { ContentfulStatusCode } from "hono/utils/http-status";
import { StatusCodes } from "http-status-codes";
import { Temporal } from "temporal-polyfill";
import { z } from "zod";
import { AppError } from "#/common/errors";
import { type AppEnv, hasFeatureFlag } from "#/env";

export type ApiSuccessResponse<T> = {
  data: T;
};

export type ApiResponse<T> = ApiSuccessResponse<T> | ApiErrorResponse;

export const ApiErrorResponse = z
  .object({
    error: z.any(),
    kind: z.string(),
    ts: z.string(),
    stackTrace: z.string().optional(),
  })
  .openapi({ ref: "ApiErrorResponse" });
export type ApiErrorResponse = z.infer<typeof ApiErrorResponse>;

export function errorHandler(e: unknown, c: Context<AppEnv>) {
  const toResponse = (
    e: Error | null,
    statusCode: ContentfulStatusCode,
    rawError: string,
    kind?: string,
  ): Response => {
    let errorsTrace = e ? new TraceableError(e).toString() : undefined;
    errorsTrace && c.env.log.debug(errorsTrace);
    if (
      !hasFeatureFlag(c.env.config.enabledFeatures, "http-error-stacktrace")
    ) {
      errorsTrace = undefined;
    }
    let error = rawError;
    // On internal error simply log the error and return a generic error so as to not leak any data.
    if (statusCode === StatusCodes.INTERNAL_SERVER_ERROR) {
      c.env.log.error(`Failed to handle request: ${JSON.stringify(e)}`);
      error = "Internal error";
    }
    const payload: ApiErrorResponse = {
      ts: Temporal.Now.instant().toString(),
      error,
      kind: kind || "INTERNAL",
      stackTrace: errorsTrace,
    };
    return c.json(payload, statusCode);
  };

  if (e instanceof AppError) {
    return toResponse(e, e.statusCode, e.message, e.kind);
  }

  if (e instanceof Error) {
    return toResponse(
      e,
      StatusCodes.INTERNAL_SERVER_ERROR,
      e.message,
      "INTERNAL",
    );
  }
  return toResponse(
    new Error(JSON.stringify(e)),
    StatusCodes.INTERNAL_SERVER_ERROR,
    "Internal error",
    "INTERNAL",
  );
}

class TraceableError {
  error: Error;

  constructor(error: Error) {
    this.error = error;
  }

  toString(): string {
    let str = "";
    let current: Error | null = this.error;

    while (current) {
      if (current.stack) {
        str += `${current.stack}\n\n`;
      }
      if (current.cause) {
        current = current.cause as Error;
        str += "Caused by:\n";
      } else {
        current = null;
      }
    }
    return str;
  }
}
