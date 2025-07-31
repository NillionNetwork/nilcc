import type { ControllerOptions } from "#/common/types";
import * as WorkloadContainerController from "./workload-container.controllers";

export function buildWorkloadContainerRouter(options: ControllerOptions): void {
  WorkloadContainerController.containerLogs(options);
  WorkloadContainerController.listContainers(options);
}
