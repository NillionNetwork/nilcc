import type { ControllerOptions } from "#/common/types";
import * as WorkloadTierController from "./workload-tier.controllers";

export function buildWorkloadTierRouter(options: ControllerOptions): void {
  WorkloadTierController.create(options);
  WorkloadTierController.list(options);
  WorkloadTierController.remove(options);
}
