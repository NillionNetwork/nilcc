import type { ContentfulStatusCode } from "hono/dist/types/utils/http-status";
import { StatusCodes } from "http-status-codes";

export abstract class AppError extends Error {
  abstract readonly kind: string;
  statusCode: ContentfulStatusCode = StatusCodes.INTERNAL_SERVER_ERROR;
  description: string | undefined;

  constructor(cause?: unknown) {
    super();
    this.cause = cause;
  }

  override get message(): string {
    return this.description || this.cause?.toString() || "Internal error";
  }
}

export class NoInstancesAvailable extends AppError {
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
  kind = "AGENT_COMMUNICATION";
  agentErrorKind: string;

  constructor(errorKind: string, message: string) {
    super();
    this.description = `agent request failed code = ${errorKind}, message = ${message}`;
    this.agentErrorKind = errorKind;
  }
}

export class EntityNotFound extends AppError {
  kind = "NOT_FOUND";
  override statusCode: ContentfulStatusCode = StatusCodes.NOT_FOUND;

  constructor(entity: string) {
    super();
    this.description = `${entity} not found`;
  }
}
