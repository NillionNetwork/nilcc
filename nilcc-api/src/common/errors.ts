import type { ContentfulStatusCode } from "hono/dist/types/utils/http-status";
import { StatusCodes } from "http-status-codes";

export abstract class AppError extends Error {
  abstract readonly kind: string;
  readonly statusCode: ContentfulStatusCode = StatusCodes.INTERNAL_SERVER_ERROR;
  readonly description: string | undefined;

  constructor(cause?: unknown) {
    super();
    this.cause = cause;
  }

  override get message(): string {
    return this.description || this.cause?.toString() || "Internal error";
  }
}

export class InstancesNotAvailable extends AppError {
  kind = "NOT_ENOUGH_RESOURCES";
  override statusCode: ContentfulStatusCode = StatusCodes.SERVICE_UNAVAILABLE;
  override description =
    "No metal instances are available to handle this workload";
}

export class InvalidDockerCompose extends AppError {
  kind = "INVALID_DOCKER_COMPOSE";
  override statusCode: ContentfulStatusCode = StatusCodes.BAD_REQUEST;
}

export class AgentRequestError extends AppError {
  kind = "METAL_INSTANCE_COMMUNICATION";
}
