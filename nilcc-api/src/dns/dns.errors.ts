import { AppError } from "#/common/errors";

export class CreateRecordError extends AppError {
  tag = "CreateRecordError";
}

export class DeleteRecordError extends AppError {
  tag = "DeleteRecordError";
}
