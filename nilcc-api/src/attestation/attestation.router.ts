import type { ControllerOptions } from "#/common/types";
import * as AttestationController from "#/attestation/attestation.controller";

export function buildAttestationRouter(options: ControllerOptions): void {
  AttestationController.verify(options);
}
