import type { ControllerOptions } from "#/common/types";
import * as PaymentController from "./payment.controller";

export function buildPaymentRouter(options: ControllerOptions): void {
  PaymentController.list(options);
}
