export type AmdFamily = "Milan" | "Genoa";

export type ReportedTcb = {
  bootloader: number;
  tee: number;
  snp: number;
  microcode: number;
};

export function deriveAmdFamily(
  cpuFamilyId: number,
  cpuModelId: number,
): AmdFamily | null {
  // Heuristic mapping based on public docs: Family 25 (0x19) with model >= 17 ~ Genoa, else Milan.
  if (cpuFamilyId === 25) {
    return cpuModelId >= 17 ? "Genoa" : "Milan";
  }
  return null;
}

export async function fetchVcekPem(
  family: AmdFamily,
  chipIdHex: string,
  tcb: ReportedTcb,
): Promise<string> {
  const base = `https://kdsintf.amd.com/vcek/v1/${family}/${chipIdHex}`;
  const url = `${base}?blSvn=${tcb.bootloader}&teeSvn=${tcb.tee}&snpSvn=${tcb.snp}&ucodeSvn=${tcb.microcode}`;
  const response = await fetch(url, { method: "GET" });
  if (!response.ok) {
    const body = await response.text();
    throw new Error(`Failed to fetch VCEK: ${response.status} ${body}`);
  }
  // AMD KDS returns a PEM-encoded certificate body
  const pem = await response.text();
  return pem;
}
