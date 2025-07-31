import { AppError } from "#/common/errors";

export class CreateRecordError extends AppError {
  kind = "CreateRecordError";
}

export class DeleteRecordError extends AppError {
  kind = "DeleteRecordError";
}
