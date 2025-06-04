import type { ControllerOptions } from "#/common/types";
import * as MetalInstanceController from "./metal-instance.controller";

export function buildMetalInstanceRouter(options: ControllerOptions): void {
  MetalInstanceController.register(options);
  MetalInstanceController.sync(options);
  MetalInstanceController.read(options);
}
