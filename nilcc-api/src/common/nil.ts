import type { ValueTransformer } from "typeorm";

export const NIL_BASE_UNITS = BigInt(10 ** 6);

/** 1 USD = 1,000,000 microdollars */
export const MICRODOLLARS_PER_USD = 1_000_000;

/** Sub-cent balances are treated as depleted (in microdollars, i.e. $0.01) */
export const MINIMUM_SPENDABLE_BALANCE = 10_000;

/** Same threshold in USD, for use in API-facing comparisons / tests */
export const MINIMUM_SPENDABLE_BALANCE_USD = 0.01;

/** TypeORM transformer for bigint columns stored as JS numbers */
export const bigintNumberTransformer: ValueTransformer = {
  to: (value: number): number => value,
  from: (value: string | number): number =>
    typeof value === "string" ? Number(value) : (value ?? 0),
};

/** Convert a uint256 NIL amount (from blockchain) to decimal NIL */
export function uint256ToNil(amount: bigint): number {
  return Number(amount) / Number(NIL_BASE_UNITS);
}

/** Convert a decimal NIL amount to integer microdollars */
export function nilToMicrodollars(
  nilAmount: number,
  nilPriceUsd: number,
): number {
  return Math.round(nilAmount * nilPriceUsd * MICRODOLLARS_PER_USD);
}

/** Convert USD (decimal) to integer microdollars */
export function usdToMicrodollars(usd: number): number {
  return Math.round(usd * MICRODOLLARS_PER_USD);
}

/** Convert integer microdollars to USD (decimal) */
export function microdollarsToUsd(microdollars: number): number {
  return microdollars / MICRODOLLARS_PER_USD;
}

/** Treat sub-cent balances as depleted (operates on microdollars) */
export function isBalanceDepleted(balanceMicrodollars: number): boolean {
  return balanceMicrodollars < MINIMUM_SPENDABLE_BALANCE;
}
