import type { ControllerOptions } from "#/common/types";
import * as AuthController from "./auth.controller";

export function buildAuthRouter(options: ControllerOptions): void {
  AuthController.challenge(options);
  AuthController.login(options);
}
