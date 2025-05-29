import { Effect as E, pipe } from "effect";
import type { Context } from "hono";
import type { ContentfulStatusCode } from "hono/utils/http-status";
import { StatusCodes } from "http-status-codes";
import { Temporal } from "temporal-polyfill";
import { z } from "zod";
import type {
  CreateEntityError,
  DataValidationError,
  FindEntityError,
  GetRepositoryError,
  RemoveEntityError,
  UpdateEntityError,
} from "#/common/errors";
import type { AppEnv } from "#/env";

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

type KnownError =
  | DataValidationError
  | GetRepositoryError
  | CreateEntityError
  | FindEntityError
  | UpdateEntityError
  | RemoveEntityError;

export function handleTaggedErrors(c: Context<AppEnv>) {
  const toResponse = (
    e: KnownError,
    statusCode: ContentfulStatusCode,
  ): E.Effect<Response> => {
    const errors = e.humanize();
    c.env.log.debug(errors);
    const payload: ApiErrorResponse = {
      ts: Temporal.Now.instant().toString(),
      errors,
    };
    return E.succeed(c.json(payload, statusCode));
  };

  return (effect: E.Effect<Response, KnownError>): E.Effect<Response> =>
    pipe(
      effect,
      E.catchTags({
        DataValidationError: (e) => toResponse(e, StatusCodes.BAD_REQUEST),
        GetRepositoryError: (e) =>
          toResponse(e, StatusCodes.INTERNAL_SERVER_ERROR),
        CreateEntityError: (e) =>
          toResponse(e, StatusCodes.INTERNAL_SERVER_ERROR),
        FindEntityError: (e) =>
          toResponse(e, StatusCodes.INTERNAL_SERVER_ERROR),
        UpdateEntityError: (e) =>
          toResponse(e, StatusCodes.INTERNAL_SERVER_ERROR),
        RemoveEntityError: (e) =>
          toResponse(e, StatusCodes.INTERNAL_SERVER_ERROR),
      }),
    );
}
