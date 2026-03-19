const COINGECKO_API_URL =
  "https://pro-api.coingecko.com/api/v3/simple/price?ids=nillion&vs_currencies=usd";

export class NilPriceService {
  constructor(private readonly apiKey: string) {}

  async fetchNilPrice(): Promise<number | null> {
    try {
      const res = await fetch(COINGECKO_API_URL, {
        headers: { "X-CG-PRO-API-KEY": this.apiKey },
      });
      if (!res.ok) {
        return null;
      }
      const data = (await res.json()) as { nillion?: { usd?: number } };
      const usd = data?.nillion?.usd;
      return typeof usd === "number" && usd > 0 ? usd : null;
    } catch {
      return null;
    }
  }
}
