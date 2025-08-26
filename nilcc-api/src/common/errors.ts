import type { ContentfulStatusCode } from "hono/dist/types/utils/http-status";
import { StatusCodes } from "http-status-codes";
import { QueryFailedError } from "typeorm";

export abstract class AppError extends Error {
  kind = "INTERNAL";
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
  override kind = "NOT_ENOUGH_RESOURCES";
  override statusCode: ContentfulStatusCode = StatusCodes.SERVICE_UNAVAILABLE;
  override description =
    "No metal instances are available to handle this workload";
}

export class MetalInstanceManagingWorkloads extends AppError {
  override kind = "METAL_INSTANCE_MANAGING_WORKLOADS";
  override statusCode: ContentfulStatusCode = StatusCodes.PRECONDITION_FAILED;
  override description = "Metal instance is handling 1 or more workloads";
}

export class InvalidDockerCompose extends AppError {
  override kind = "INVALID_DOCKER_COMPOSE";
  override statusCode: ContentfulStatusCode = StatusCodes.BAD_REQUEST;
}

export class InvalidWorkloadTier extends AppError {
  override kind = "INVALID_WORKLOAD_TIER";
  override statusCode: ContentfulStatusCode = StatusCodes.BAD_REQUEST;
  override description =
    "no matching workload tier for the requested resources";
}

export class NotEnoughCredits extends AppError {
  override kind = "NOT_ENOUGH_CREDITS";
  override statusCode: ContentfulStatusCode = StatusCodes.PRECONDITION_FAILED;
  override description = "not enough credits in account to run workload";
}

export class AgentRequestError extends AppError {
  override kind = "AGENT_COMMUNICATION";
  agentErrorKind: string;
  agentErrorDescription: string;

  constructor(errorKind: string, message: string) {
    super();
    this.description = `agent request failed code = ${errorKind}, message = ${message}`;
    this.agentErrorKind = errorKind;
    this.agentErrorDescription = message;
  }
}

export class AgentCreateWorkloadError extends AppError {
  override statusCode: ContentfulStatusCode = StatusCodes.BAD_REQUEST;

  constructor(errorKind: string, message: string) {
    super();
    this.kind = errorKind;
    this.description = message;
  }
}

export class EntityNotFound extends AppError {
  override kind = "NOT_FOUND";
  override statusCode: ContentfulStatusCode = StatusCodes.NOT_FOUND;

  constructor(entity: string) {
    super();
    this.description = `${entity} not found`;
  }
}

export class EntityAlreadyExists extends AppError {
  override kind = "ALREADY_EXISTS";
  override statusCode: ContentfulStatusCode = StatusCodes.CONFLICT;

  constructor(entity: string) {
    super();
    this.description = `${entity} already exists`;
  }
}

export class AccessDenied extends AppError {
  override kind = "ACCESS_DENIED";
  override statusCode: ContentfulStatusCode = StatusCodes.UNAUTHORIZED;

  constructor() {
    super();
    this.description = "access denied";
  }
}

export function isUniqueConstraint(e: unknown): boolean {
  return e instanceof QueryFailedError && e.driverError.code === "23505";
}
