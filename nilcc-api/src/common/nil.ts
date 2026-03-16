export const NIL_BASE_UNITS = BigInt(10 ** 6);

/** Convert a uint256 NIL amount (from blockchain) to decimal NIL */
export function uint256ToNil(amount: bigint): number {
  return Number(amount) / Number(NIL_BASE_UNITS);
}

/** Convert a USD/min rate to NIL per minute given the current NIL price in USD */
export function usdToNil(usdPerMin: number, nilPriceUsd: number): number {
  return usdPerMin / nilPriceUsd;
}
