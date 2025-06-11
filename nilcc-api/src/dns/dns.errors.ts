import { AppError } from "#/common/errors";

export class RegisterCnameError extends AppError {
  tag = "RegisterCnameError";
}

export class RemoveDomainError extends AppError {
  tag = "RemoveDomainError";
}
