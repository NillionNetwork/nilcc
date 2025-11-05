import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { adminOrUserAuthentication } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator, responseValidator } from "#/common/zod-utils";
import { AttestationService } from "#/attestation/attestation.service";
import {
  VerifySnpAttestationRequest,
  VerifySnpAttestationResponse,
} from "#/attestation/attestation.dto";

export function verify(options: ControllerOptions) {
  const { app, bindings } = options;
  const service = new AttestationService(bindings.log);

  app.post(
    PathsV1.attestation.verify,
    describeRoute({
      tags: ["attestation"],
      summary: "Verify AMD SEV-SNP attestation signature",
      description:
        "Verifies an AMD SEV-SNP attestation report signature using a provided VCEK certificate, or by fetching the VCEK from AMD KDS using chipId and reported TCB.",
      responses: {
        200: {
          description: "Verification result",
          content: {
            "application/json": {
              schema: resolver(VerifySnpAttestationResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    adminOrUserAuthentication(bindings),
    payloadValidator(VerifySnpAttestationRequest),
    responseValidator(bindings, VerifySnpAttestationResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const result = await service.verifySnp(payload);
      return c.json(result);
    },
  );
}
