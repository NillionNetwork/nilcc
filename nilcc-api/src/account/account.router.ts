import type { ControllerOptions } from "#/common/types";
import * as AccountController from "./account.controller";

export function buildAccountRouter(options: ControllerOptions): void {
  AccountController.create(options);
  AccountController.update(options);
  AccountController.list(options);
  AccountController.me(options);
  AccountController.read(options);
  AccountController.addCredits(options);
}
