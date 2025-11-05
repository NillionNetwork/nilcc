export type SnpExtensions = {
  bootloader?: number;
  tee?: number;
  snp?: number;
  microcode?: number;
  hwid?: Uint8Array; // 64 bytes
  fmc?: number;
};

// AMD SNP OIDs per sev crate usage
const OID = {
  BootLoader: "1.3.6.1.4.1.3704.1.3.1",
  Tee: "1.3.6.1.4.1.3704.1.3.2",
  Snp: "1.3.6.1.4.1.3704.1.3.3",
  Ucode: "1.3.6.1.4.1.3704.1.3.8",
  HwId: "1.3.6.1.4.1.3704.1.4",
  Fmc: "1.3.6.1.4.1.3704.1.3.9",
} as const;

export async function parseVcekExtensions(
  _vcekPem: string,
): Promise<SnpExtensions | null> {
  // Placeholder: implement robust X.509 parsing and OID extraction using a library.
  // For now, return null so that extension comparison is skipped when not supported.
  return null;
}

function extractOctetOrRaw(
  src: Uint8Array,
  expectedLen: number,
): Uint8Array | null {
  // Heuristic: if the value looks like DER OCTET STRING (0x04, len), peel it, else try to match tail
  if (src.length >= 2 && src[0] === 0x04) {
    const lenByte = src[1];
    if ((lenByte & 0x80) === 0) {
      const len = lenByte;
      if (2 + len <= src.length) {
        return src.subarray(2, 2 + len);
      }
    } else {
      const n = lenByte & 0x7f;
      let len = 0;
      for (let i = 0; i < n; i++) len = (len << 8) | src[2 + i];
      const start = 2 + n;
      if (start + len <= src.length) return src.subarray(start, start + len);
    }
  }
  // Otherwise, return either the whole value or the last expectedLen bytes when too large
  if (src.length === expectedLen) return src;
  if (src.length > expectedLen) return src.subarray(src.length - expectedLen);
  return null;
}

export function hexToBytes(hex: string): Uint8Array {
  const clean = hex.replace(/^0x/, "");
  if (clean.length % 2 !== 0) throw new Error("invalid hex length");
  const out = new Uint8Array(clean.length / 2);
  for (let i = 0; i < out.length; i++)
    out[i] = parseInt(clean.slice(i * 2, i * 2 + 2), 16);
  return out;
}
