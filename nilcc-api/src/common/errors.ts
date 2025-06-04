import { ZodError } from "zod";
import { fromZodError } from "zod-validation-error";

function isError(error: unknown): error is Error {
  return error instanceof Error;
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
            throw map(error);
          });
        }
        return result;
      } catch (error) {
        if (isError(error)) {
          throw map(error);
        }

        throw map(new Error(`Unexpected error: ${error}`));
      }
    };
    return descriptor;
  };
}

abstract class AppError {
  abstract readonly tag: string;
  cause?: unknown;
  constructor({ cause }: { cause?: unknown }) {
    this.cause = cause;
  }

  humanize(): string[] {
    if (this.cause instanceof AppError) {
      return [this.tag, ...this.cause.humanize()];
    }
    if (isError(this.cause)) {
      return [this.tag, `Cause: ${this.cause}`];
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
    super({ cause });
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

export class CreateEntityError extends AppError {
  tag = "CreateEntityError";
}

export class CreateOrUpdateEntityError extends AppError {
  tag = "CreateOrUpdateEntityError";
}

export class FindEntityError extends AppError {
  tag = "FindEntityError";
}

export class UpdateEntityError extends AppError {
  tag = "UpdateEntityError";
}

export class RemoveEntityError extends AppError {
  tag = "RemoveEntityError";
}
