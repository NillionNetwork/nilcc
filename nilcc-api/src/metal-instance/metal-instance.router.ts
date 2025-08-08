import type { ControllerOptions } from "#/common/types";
import * as MetalInstanceController from "./metal-instance.controller";

export function buildMetalInstanceRouter(options: ControllerOptions): void {
  MetalInstanceController.heartbeat(options);
  MetalInstanceController.list(options);
  MetalInstanceController.read(options);
  MetalInstanceController.register(options);
  MetalInstanceController.remove(options);
}
