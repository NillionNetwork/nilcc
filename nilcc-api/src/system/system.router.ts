import type { ControllerOptions } from "#/common/types";
import * as SystemController from "./system.controllers";

export function buildSystemRouter(options: ControllerOptions): void {
  SystemController.health(options);
}
