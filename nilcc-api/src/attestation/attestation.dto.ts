import { z } from "zod";

export const VerifySnpAttestationRequest = z
  .object({
    reportBinary: z.string().min(1).openapi({
      description: "Base64-encoded raw AMD SEV-SNP attestation report bytes",
    }),
    vcekPem: z.string().optional().openapi({
      description: "PEM-encoded AMD SEV-SNP VCEK certificate (optional)",
    }),
    chipId: z
      .string()
      .regex(/^[0-9a-fA-F]+$/)
      .length(128)
      .optional()
      .openapi({
        description:
          "Hex-encoded chip ID (required if vcekPem is not provided)",
      }),
    reportedTcb: z
      .object({
        bootloader: z.number().int().nonnegative(),
        tee: z.number().int().nonnegative(),
        snp: z.number().int().nonnegative(),
        microcode: z.number().int().nonnegative(),
      })
      .optional()
      .openapi({
        description:
          "Reported TCB used to request VCEK (required if vcekPem is not provided)",
      }),
    cpu: z
      .object({
        familyId: z.number().int().nonnegative(),
        modelId: z.number().int().nonnegative(),
      })
      .optional()
      .openapi({
        description:
          "CPU identifiers to derive AMD family when fetching VCEK (optional)",
      }),
    family: z
      .enum(["Milan", "Genoa"]) // Extend as needed
      .optional()
      .openapi({
        description: "Override AMD family name used for VCEK lookup (optional)",
      }),
  })
  .openapi({ ref: "VerifySnpAttestationRequest" });
export type VerifySnpAttestationRequest = z.infer<
  typeof VerifySnpAttestationRequest
>;

export const VerifySnpAttestationResponse = z
  .object({
    valid: z.boolean(),
    error: z.string().optional(),
  })
  .openapi({ ref: "VerifySnpAttestationResponse" });
export type VerifySnpAttestationResponse = z.infer<
  typeof VerifySnpAttestationResponse
>;
