import type { ControllerOptions } from "#/common/types";
import * as ApiKeyController from "./api-key.controller";

export function buildApiKeyRouter(options: ControllerOptions): void {
  ApiKeyController.create(options);
  ApiKeyController.listByAccount(options);
  ApiKeyController.update(options);
  ApiKeyController.remove(options);
}
