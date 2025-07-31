import type { ControllerOptions } from "#/common/types";
import * as WorkloadEventController from "./workload-event.controllers";

export function buildWorkloadEventRouter(options: ControllerOptions): void {
  WorkloadEventController.submitEvent(options);
  WorkloadEventController.listEvents(options);
}
