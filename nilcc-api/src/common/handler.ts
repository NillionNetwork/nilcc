import type { Context } from "hono";
import type { ContentfulStatusCode } from "hono/utils/http-status";
import { StatusCodes } from "http-status-codes";
import { Temporal } from "temporal-polyfill";
import { z } from "zod";
import {
  CreateEntityError,
  DataValidationError,
  FindEntityError,
  GetRepositoryError,
  HttpError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";

export type ApiSuccessResponse<T> = {
  data: T;
};

export type ApiResponse<T> = ApiSuccessResponse<T> | ApiErrorResponse;

export const ApiErrorResponse = z
  .object({
    errors: z.array(z.string()),
    ts: z.string(),
  })
  .openapi({ ref: "ApiErrorResponse" });
export type ApiErrorResponse = z.infer<typeof ApiErrorResponse>;

export function errorHandler(e: unknown, c: Context) {
  const toResponse = (
    e: Error | null,
    errors: string[],
    statusCode: ContentfulStatusCode,
  ): Response => {
    e && c.env.log.debug(e);
    const payload: ApiErrorResponse = {
      ts: Temporal.Now.instant().toString(),
      errors,
    };
    return c.json(payload, statusCode);
  };

  if (e instanceof DataValidationError) {
    return toResponse(e, e.humanize(), StatusCodes.BAD_REQUEST);
  }

  if (e instanceof HttpError) {
    return toResponse(e, e.humanize(), e.statusCode);
  }

  if (
    e instanceof GetRepositoryError ||
    e instanceof CreateEntityError ||
    e instanceof FindEntityError ||
    e instanceof UpdateEntityError ||
    e instanceof RemoveEntityError
  ) {
    return toResponse(e, e.humanize(), StatusCodes.INTERNAL_SERVER_ERROR);
  }

  if (e instanceof Error) {
    return toResponse(e, [e.message], StatusCodes.INTERNAL_SERVER_ERROR);
  }
  // Default error
  return toResponse(null, ["Unknown Error"], StatusCodes.INTERNAL_SERVER_ERROR);
}
