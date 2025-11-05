import crypto from "node:crypto";
import { afterEach, describe, expect, it, vi } from "vitest";
import { VerifySnpAttestationRequest } from "#/attestation/attestation.dto";
import { buildFixture } from "./fixture/fixture";

function derToFixedRS(der: Buffer): { r: Buffer; s: Buffer } {
  let offset = 0;
  if (der[offset++] !== 0x30) throw new Error("bad der: seq");
  const lenByte = der[offset++];
  if (lenByte & 0x80) {
    const numLenBytes = lenByte & 0x7f;
    offset += numLenBytes;
  }
  if (der[offset++] !== 0x02) throw new Error("bad der: r int tag");
  const rLen = der[offset++];
  let r = der.subarray(offset, offset + rLen);
  offset += rLen;
  if (der[offset++] !== 0x02) throw new Error("bad der: s int tag");
  const sLen = der[offset++];
  let s = der.subarray(offset, offset + sLen);
  // Trim left 0x00 padding and left-pad to 48 bytes
  r = trimLeftZeros(r);
  s = trimLeftZeros(s);
  return { r: leftPad(r, 48), s: leftPad(s, 48) };
}

function trimLeftZeros(b: Buffer): Buffer {
  let i = 0;
  while (i < b.length - 1 && b[i] === 0x00) i++;
  return b.subarray(i);
}

function leftPad(b: Buffer, size: number): Buffer {
  if (b.length >= size) return b;
  const out = Buffer.alloc(size);
  b.copy(out, size - b.length);
  return out;
}

describe("Attestation verification", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("verifies a valid signature when VCEK PEM is provided", async () => {
    const { clients } = await buildFixture();

    // Generate P-384 key pair
    const { publicKey, privateKey } = crypto.generateKeyPairSync("ec", {
      namedCurve: "secp384r1",
    });
    const vcekPem = publicKey
      .export({ type: "spki", format: "pem" })
      .toString();

    // Create synthetic report: signedRegion || (r||s)
    const totalLen = 200;
    const sigLen = 96; // r||s (each 48 bytes)
    const signedLen = totalLen - sigLen;
    const signedRegion = crypto.randomBytes(signedLen);
    let report = Buffer.concat([signedRegion, Buffer.alloc(sigLen, 0)]);

    // Sign the signed region
    const sign = crypto.createSign("sha384");
    sign.update(signedRegion);
    sign.end();
    const derSig = sign.sign(privateKey);
    const { r, s } = derToFixedRS(derSig);
    report = Buffer.concat([signedRegion, r, s]);

    const body: VerifySnpAttestationRequest = {
      reportBinary: report.toString("base64"),
      vcekPem,
    };

    const result = await clients.user.verifyAttestation(body).submit();
    expect(result.valid).toBe(true);
  });

  it("rejects an invalid signature", async () => {
    const { clients } = await buildFixture();

    const { publicKey, privateKey } = crypto.generateKeyPairSync("ec", {
      namedCurve: "secp384r1",
    });
    const vcekPem = publicKey
      .export({ type: "spki", format: "pem" })
      .toString();

    const totalLen = 200;
    const sigLen = 96;
    const signedLen = totalLen - sigLen;
    const signedRegion = crypto.randomBytes(signedLen);
    let report = Buffer.concat([signedRegion, Buffer.alloc(sigLen, 0)]);

    const sign = crypto.createSign("sha384");
    sign.update(signedRegion);
    sign.end();
    const derSig = sign.sign(privateKey);
    let { r, s } = derToFixedRS(derSig);
    // Corrupt r
    r = Buffer.from(r);
    r[0] ^= 0xff;
    report = Buffer.concat([signedRegion, r, s]);

    const body: VerifySnpAttestationRequest = {
      reportBinary: report.toString("base64"),
      vcekPem,
    };

    const result = await clients.user.verifyAttestation(body).submit();
    expect(result.valid).toBe(false);
  });

  it("fetches VCEK from AMD KDS when not provided", async () => {
    const { clients } = await buildFixture();

    const { publicKey, privateKey } = crypto.generateKeyPairSync("ec", {
      namedCurve: "secp384r1",
    });
    const vcekPem = publicKey
      .export({ type: "spki", format: "pem" })
      .toString();

    const totalLen = 200;
    const sigLen = 96;
    const signedLen = totalLen - sigLen;
    const signedRegion = crypto.randomBytes(signedLen);
    let report = Buffer.concat([signedRegion, Buffer.alloc(sigLen, 0)]);

    const sign = crypto.createSign("sha384");
    sign.update(signedRegion);
    sign.end();
    const derSig = sign.sign(privateKey);
    const { r, s } = derToFixedRS(derSig);
    report = Buffer.concat([signedRegion, r, s]);

    // Mock fetch to AMD KDS
    const originalFetch = globalThis.fetch;
    vi.spyOn(globalThis as any, "fetch").mockImplementation(
      async (input: any) => {
        const url = String(input);
        if (url.startsWith("https://kdsintf.amd.com/vcek/v1/")) {
          return {
            ok: true,
            status: 200,
            text: async () => vcekPem,
          } as unknown as Response;
        }
        return originalFetch(input);
      },
    );

    const body: VerifySnpAttestationRequest = {
      reportBinary: report.toString("base64"),
      chipId: "a".repeat(128),
      reportedTcb: { bootloader: 9, tee: 0, snp: 23, microcode: 72 },
      family: "Genoa",
    };

    const result = await clients.user.verifyAttestation(body).submit();
    expect(result.valid).toBe(true);
  });
});
