import { AppError } from "#/common/errors";

export class CreateRecordError extends AppError {
  override kind = "CreateRecordError";
}

export class DeleteRecordError extends AppError {
  override kind = "DeleteRecordError";
}
