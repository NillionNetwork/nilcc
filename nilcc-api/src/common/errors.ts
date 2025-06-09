import type { ContentfulStatusCode } from "hono/dist/types/utils/http-status";
import { ZodError } from "zod";
import { fromZodError } from "zod-validation-error";

function isError(error: unknown): error is Error {
  return error instanceof Error;
}

function handleError<T extends AppError>(
  error: unknown,
  map: (error: Error) => T,
) {
  // If the error is an Error we can map it to a specific AppError
  if (isError(error)) {
    throw map(error);
  }
  // If the error is not an Error we create an error with the message using the error and hope it to be printable
  throw map(new Error(`Unexpected error: ${error}`));
}

export function mapError<T extends AppError>(map: (error: Error) => T) {
  return (
    _target: unknown,
    _propertyKey: string,
    descriptor: PropertyDescriptor,
  ) => {
    const originalMethod = descriptor.value;
    descriptor.value = function (...args: unknown[]) {
      try {
        const result = originalMethod.apply(this, args);

        if (result instanceof Promise) {
          return result.catch((error) => {
            handleError(error, map);
          });
        }
        return result;
      } catch (error) {
        handleError(error, map);
      }
    };
    return descriptor;
  };
}

abstract class AppError extends Error {
  abstract readonly tag: string;
  constructor(cause?: unknown) {
    super();
    this.cause = cause;
  }

  humanize(): string[] {
    if (this.cause instanceof AppError) {
      return [this.tag, ...this.cause.humanize()];
    }
    if (isError(this.cause)) {
      return [this.tag, `Cause: ${this.cause.message}`];
    }
    return [this.tag, `Cause: ${this.cause}`];
  }
}

export class DataValidationError extends AppError {
  tag = "DataValidationError";
  issues: (string | ZodError)[];

  constructor({
    cause,
    issues,
  }: { cause?: unknown; issues: (string | ZodError)[] }) {
    super(cause);
    this.issues = issues;
  }

  override humanize(): string[] {
    const flattenedIssues = this.issues.flatMap((issue) => {
      if (issue instanceof ZodError) {
        const errorMessage = fromZodError(issue, {
          prefix: null,
          issueSeparator: ";",
        }).message;
        return errorMessage.split(";");
      }
      return issue;
    });

    return [this.tag, ...flattenedIssues];
  }
}

export class GetRepositoryError extends AppError {
  tag = "GetRepositoryError";
}

abstract class EntityError<T extends object> extends AppError {
  entity: T;

  constructor(entity: T, cause?: unknown) {
    super(cause);
    this.entity = entity;
  }

  override humanize(): string[] {
    const baseMessage = super.humanize();
    delete baseMessage[0];
    return [`${this.tag}<${this.entity.constructor.name}>`, ...baseMessage];
  }
}

export class CreateEntityError<T extends object> extends EntityError<T> {
  tag = "CreateEntityError";
}

export class CreateOrUpdateEntityError<
  T extends object,
> extends EntityError<T> {
  tag = "CreateOrUpdateEntityError";
}

export class FindEntityError<T extends object> extends EntityError<T> {
  tag = "FindEntityError";
}

export class UpdateEntityError<T extends object> extends EntityError<T> {
  tag = "UpdateEntityError";
}

export class RemoveEntityError<T extends object> extends EntityError<T> {
  tag = "RemoveEntityError";
}

export class InstancesNotAvailable extends AppError {
  tag = "InstancesNotAvailable";
}

export class HttpError extends AppError {
  tag = "HttpError";
  statusCode: ContentfulStatusCode;

  constructor({
    message,
    statusCode,
    cause,
  }: { message: string; statusCode: ContentfulStatusCode; cause?: unknown }) {
    super(cause);
    this.statusCode = statusCode;
    this.message = message;
  }

  override humanize(): string[] {
    return [this.message];
  }
}
