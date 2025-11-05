import crypto from "node:crypto";
import type { Logger } from "pino";
import type { VerifySnpAttestationRequest } from "#/attestation/attestation.dto";
import {
  deriveAmdFamily,
  fetchVcekPem,
  type AmdFamily,
} from "#/attestation/amd-kds.client";
import { parseVcekExtensions, hexToBytes } from "#/attestation/cert-utils";

export class AttestationService {
  log: Logger;

  constructor(log: Logger) {
    this.log = log;
  }

  async verifySnp(
    req: VerifySnpAttestationRequest,
  ): Promise<{ valid: boolean; error?: string }> {
    try {
      const report = Buffer.from(req.reportBinary, "base64");
      if (report.length < 128) {
        return { valid: false, error: "reportBinary too small" };
      }

      // AMD SEV-SNP attestation report signature is ECDSA P-384 over SHA-384 of the report content excluding the signature block (96 bytes: r||s)
      const SIGNATURE_RS_LEN = 96; // 2 * 48 bytes
      if (report.length <= SIGNATURE_RS_LEN) {
        return { valid: false, error: "reportBinary length invalid" };
      }
      const signedRegion = report.subarray(0, report.length - SIGNATURE_RS_LEN);
      const rsRegion = report.subarray(report.length - SIGNATURE_RS_LEN);
      const rBytes = rsRegion.subarray(0, 48);
      const sBytes = rsRegion.subarray(48, 96);
      const derSignature = this.toDerSignatureFromBytes(rBytes, sBytes);

      const vcekPem = await this.resolveVcekPem(req);
      // Optional: compare VCEK extensions with provided TCB/HWID (when using KDS or when caller supplied a real VCEK cert)
      if (req.chipId && req.reportedTcb) {
        const exts = await parseVcekExtensions(vcekPem);
        if (exts) {
          const mismatches: string[] = [];
          if (
            exts.bootloader !== undefined &&
            exts.bootloader !== req.reportedTcb.bootloader
          )
            mismatches.push("bootloader");
          if (exts.tee !== undefined && exts.tee !== req.reportedTcb.tee)
            mismatches.push("tee");
          if (exts.snp !== undefined && exts.snp !== req.reportedTcb.snp)
            mismatches.push("snp");
          if (
            exts.microcode !== undefined &&
            exts.microcode !== req.reportedTcb.microcode
          )
            mismatches.push("microcode");
          if (exts.hwid) {
            const hwid = hexToBytes(req.chipId);
            if (hwid.length === 64) {
              let equal = true;
              for (let i = 0; i < 64; i++)
                if (exts.hwid[i] !== hwid[i]) {
                  equal = false;
                  break;
                }
              if (!equal) mismatches.push("hwid");
            }
          }
          if (mismatches.length > 0) {
            return {
              valid: false,
              error: `VCEK extension mismatch: ${mismatches.join(",")}`,
            };
          }
        }
      }
      const publicKey = crypto.createPublicKey(vcekPem);

      const verifier = crypto.createVerify("sha384");
      verifier.update(signedRegion);
      verifier.end();
      const ok = verifier.verify(publicKey, derSignature);
      return { valid: ok, error: ok ? undefined : "invalid signature" };
    } catch (e: unknown) {
      const error = e instanceof Error ? e.message : String(e);
      this.log.error(`Attestation verification failed: ${error}`);
      return { valid: false, error };
    }
  }

  private async resolveVcekPem(
    req: VerifySnpAttestationRequest,
  ): Promise<string> {
    if (req.vcekPem) {
      return req.vcekPem;
    }
    if (!req.chipId || !req.reportedTcb) {
      throw new Error("vcekPem not provided and chipId/reportedTcb missing");
    }
    let family: AmdFamily | null = null;
    if (req.family) {
      family = req.family;
    } else if (req.cpu) {
      family = deriveAmdFamily(req.cpu.familyId, req.cpu.modelId);
    }
    if (!family) {
      // Default to Genoa if unknown, as a pragmatic fallback
      family = "Genoa";
    }
    return await fetchVcekPem(family, req.chipId, req.reportedTcb);
  }

  private toDerSignatureFromBytes(r: Buffer, s: Buffer): Buffer {
    const rT = this.trimLeftZeros(Buffer.from(r));
    const sT = this.trimLeftZeros(Buffer.from(s));
    const rEnc = this.encodeDerInteger(rT);
    const sEnc = this.encodeDerInteger(sT);
    const len = rEnc.length + sEnc.length;
    const lenBytes = this.encodeDerLength(len);
    return Buffer.concat([Buffer.from([0x30]), lenBytes, rEnc, sEnc]);
  }

  private encodeDerInteger(buf: Buffer): Buffer {
    // Prepend 0x00 if high bit is set to ensure positive INTEGER
    const needsPad = buf.length > 0 && (buf[0] & 0x80) !== 0;
    const val = needsPad ? Buffer.concat([Buffer.from([0x00]), buf]) : buf;
    const len = this.encodeDerLength(val.length);
    return Buffer.concat([Buffer.from([0x02]), len, val]); // INTEGER tag = 0x02
  }

  private encodeDerLength(length: number): Buffer {
    if (length < 0x80) {
      return Buffer.from([length]);
    }
    const bytes: number[] = [];
    let tmp = length;
    while (tmp > 0) {
      bytes.unshift(tmp & 0xff);
      tmp >>= 8;
    }
    return Buffer.from([0x80 | bytes.length, ...bytes]);
  }

  private trimLeftZeros(buf: Buffer): Buffer {
    let i = 0;
    while (i < buf.length - 1 && buf[i] === 0x00) {
      i++;
    }
    return buf.subarray(i);
  }
}
