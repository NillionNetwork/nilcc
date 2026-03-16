export const NIL_BASE_UNITS = BigInt(10 ** 6);
export const MINIMUM_SPENDABLE_BALANCE_NIL = 0.01;

/** Convert a uint256 NIL amount (from blockchain) to decimal NIL */
export function uint256ToNil(amount: bigint): number {
  return Number(amount) / Number(NIL_BASE_UNITS);
}

/** Convert a USD/min rate to NIL per minute given the current NIL price in USD */
export function usdToNil(usdPerMin: number, nilPriceUsd: number): number {
  return usdPerMin / nilPriceUsd;
}

/** Treat sub-cent NIL balances as depleted so workloads stop on the current heartbeat. */
export function isBalanceDepleted(balanceNil: number): boolean {
  return balanceNil < MINIMUM_SPENDABLE_BALANCE_NIL;
}
