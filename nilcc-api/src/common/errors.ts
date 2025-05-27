import { Data } from "effect";
import { ZodError } from "zod";
import { fromZodError } from "zod-validation-error";

export class DataValidationError extends Data.TaggedError(
  "DataValidationError",
)<{
  issues: (string | ZodError)[];
  cause: unknown;
}> {
  humanize(): string[] {
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

    return [this._tag, ...flattenedIssues];
  }
}

export class GetRepositoryError extends Data.TaggedError("GetRepositoryError")<{
  cause: unknown;
}> {
  humanize(): string[] {
    return [this._tag, `Cause: ${this.cause}`];
  }
}

export class CreateEntityError extends Data.TaggedError("CreateEntityError")<{
  cause: unknown;
}> {
  humanize(): string[] {
    return [this._tag, `Cause: ${this.cause}`];
  }
}

export class FindEntityError extends Data.TaggedError("FindEntityError")<{
  cause: unknown;
}> {
  humanize(): string[] {
    return [this._tag, `Cause: ${this.cause}`];
  }
}

export class UpdateEntityError extends Data.TaggedError("UpdateEntityError")<{
  cause: unknown;
}> {
  humanize(): string[] {
    return [this._tag, `Cause: ${this.cause}`];
  }
}

export class RemoveEntityError extends Data.TaggedError("RemoveEntityError")<{
  cause: unknown;
}> {
  humanize(): string[] {
    return [this._tag, `Cause: ${this.cause}`];
  }
}
