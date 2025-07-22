import type { ControllerOptions } from "#/common/types";
import * as WorkloadController from "./workload.controllers";

export function buildWorkloadRouter(options: ControllerOptions): void {
  WorkloadController.create(options);
  WorkloadController.list(options);
  WorkloadController.read(options);
  WorkloadController.remove(options);
  WorkloadController.submitEvent(options);
  WorkloadController.listEvents(options);
}
