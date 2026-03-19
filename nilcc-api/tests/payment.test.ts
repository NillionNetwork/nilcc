import * as crypto from "node:crypto";
import { describe, vi } from "vitest";
import { PathsV1 } from "#/common/paths";
import type { PaymentListResponse } from "#/payment/payment.dto";
import { PaymentEntity } from "#/payment/payment.entity";
import { createTestFixtureExtension } from "./fixture/it";

describe("Payment", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  describe("PaymentService.processEvent", () => {
    it("should credit an account for a valid payment event", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "payment-test",
          walletAddress,
          balance: 0,
        })
        .submit();

      const payment = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 1000,
        fromAddress: walletAddress,
        amount: BigInt(2) * BigInt(10 ** 6), // 2 tokens
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(payment).not.toBeNull();
      expect(payment?.nilAmount).toBe(2); // 2 tokens = 2 NIL
      expect(payment?.nilPriceAtDeposit).toBe(1.0);
      expect(payment?.depositedAmountUsd).toBe(2_000_000); // 2 NIL @ $1.0 = $2.00 = 2M microdollars

      // Verify account balance was updated in USD
      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.balance).toBe(2);
    });

    it("should be idempotent for duplicate txHash", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "idempotent-test",
          walletAddress,
          balance: 0,
        })
        .submit();

      const txHash = `0x${crypto.randomBytes(32).toString("hex")}`;
      const event = {
        txHash,
        logIndex: 0,
        blockNumber: 2000,
        fromAddress: walletAddress,
        amount: BigInt(10 ** 6), // 1 token
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      };

      // Process the same event twice
      const first = await bindings.services.payment.processEvent(
        bindings,
        event,
      );
      const second = await bindings.services.payment.processEvent(
        bindings,
        event,
      );

      expect(first).not.toBeNull();
      expect(second).not.toBeNull();
      expect(first?.id).toBe(second?.id);

      // Balance should only be applied once (1 token = 1 NIL = $1 USD at price 1.0)
      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.balance).toBe(1);
    });

    it("should return null for unknown wallet address", async ({
      expect,
      bindings,
    }) => {
      const unknownWallet = `0x${crypto.randomBytes(20).toString("hex")}`;
      const result = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 3000,
        fromAddress: unknownWallet,
        amount: BigInt(10 ** 6),
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(result).toBeNull();
    });

    it("should credit fractional-token amounts", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "fractional-credit-test",
          walletAddress,
          balance: 0,
        })
        .submit();

      const result = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 4000,
        fromAddress: walletAddress,
        amount: BigInt(10 ** 5), // 0.1 token = 100000 base units
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(result).not.toBeNull();
      expect(result?.nilAmount).toBe(0.1);
      expect(result?.depositedAmountUsd).toBe(100_000); // 0.1 NIL @ $1.0 = $0.10 = 100k microdollars

      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.balance).toBe(0.1);
    });

    it("should still deposit very small amounts", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "small-amount-test",
          walletAddress,
          balance: 0,
        })
        .submit();

      const result = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 4001,
        fromAddress: walletAddress,
        amount: BigInt(10 ** 2), // 100 base units = 0.0001 NIL = 100 microdollars
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(result).not.toBeNull();
      expect(result?.nilAmount).toBe(0.0001);
      // 0.0001 NIL @ $1.0 = 100 microdollars
      expect(result?.depositedAmountUsd).toBe(100);

      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.balance).toBe(0.0001); // 100 microdollars = $0.0001
    });

    it("should retry when the NIL price is unavailable", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "nil-price-unavailable",
          walletAddress,
          balance: 0,
        })
        .submit();

      const txHash = `0x${crypto.randomBytes(32).toString("hex")}`;
      const priceSpy = vi
        .spyOn(bindings.services.nilPrice, "fetchNilPrice")
        .mockResolvedValueOnce(null);

      await expect(
        bindings.services.payment.processEvent(bindings, {
          txHash,
          logIndex: 0,
          blockNumber: 4002,
          fromAddress: walletAddress,
          amount: BigInt(10 ** 6),
          digest: `0x${crypto.randomBytes(32).toString("hex")}`,
        }),
      ).rejects.toThrow(/NIL price unavailable/);

      const payment = await bindings.dataSource
        .getRepository(PaymentEntity)
        .findOneBy({ txHash });
      expect(payment).toBeNull();

      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.balance).toBe(0);

      priceSpy.mockRestore();
    });
  });

  describe("GET /api/v1/payments/list", () => {
    it("should require authentication", async ({ expect, app }) => {
      const response = await app.request(PathsV1.payments.list, {
        method: "GET",
      });

      expect(response.status).toBe(401);
    });

    it("should return an empty list when no payments exist", async ({
      expect,
      app,
      clients,
      issueJwt,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "empty-payments",
          walletAddress,
          balance: 0,
        })
        .submit();

      const jwt = await issueJwt(account.accountId, walletAddress);
      const response = await app.request(PathsV1.payments.list, {
        method: "GET",
        headers: { authorization: `Bearer ${jwt}` },
      });

      expect(response.status).toBe(200);
      const payments = (await response.json()) as PaymentListResponse;
      expect(payments).toEqual([]);
    });

    it("should return payments for the authenticated account", async ({
      expect,
      app,
      bindings,
      clients,
      issueJwt,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "with-payments",
          walletAddress,
          balance: 0,
        })
        .submit();

      // Process a payment
      const txHash = `0x${crypto.randomBytes(32).toString("hex")}`;
      await bindings.services.payment.processEvent(bindings, {
        txHash,
        logIndex: 0,
        blockNumber: 5000,
        fromAddress: walletAddress,
        amount: BigInt(3) * BigInt(10 ** 6),
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      const jwt = await issueJwt(account.accountId, walletAddress);
      const response = await app.request(PathsV1.payments.list, {
        method: "GET",
        headers: { authorization: `Bearer ${jwt}` },
      });

      expect(response.status).toBe(200);
      const payments = (await response.json()) as PaymentListResponse;
      expect(payments).toHaveLength(1);
      expect(payments[0].txHash).toBe(txHash);
      expect(payments[0].blockNumber).toBe(5000);
      expect(payments[0].fromAddress).toBe(walletAddress.toLowerCase());
      expect(payments[0].depositedAmountUsd).toBe(3); // 3 NIL @ $1.0 = $3.00
      expect(payments[0].paymentId).toBeDefined();
      expect(payments[0].createdAt).toBeDefined();
    });

    it("should not return payments for other accounts", async ({
      expect,
      app,
      bindings,
      clients,
      issueJwt,
    }) => {
      // Create two accounts
      const wallet1 = `0x${crypto.randomBytes(20).toString("hex")}`;
      const wallet2 = `0x${crypto.randomBytes(20).toString("hex")}`;
      await clients.admin
        .createAccount({
          name: "pay-owner",
          walletAddress: wallet1,
          balance: 0,
        })
        .submit();
      const account2 = await clients.admin
        .createAccount({
          name: "pay-other",
          walletAddress: wallet2,
          balance: 0,
        })
        .submit();

      // Process a payment for account1
      await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 6000,
        fromAddress: wallet1,
        amount: BigInt(10 ** 6),
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      // Account2 should see no payments
      const jwt2 = await issueJwt(account2.accountId, wallet2);
      const response = await app.request(PathsV1.payments.list, {
        method: "GET",
        headers: { authorization: `Bearer ${jwt2}` },
      });

      expect(response.status).toBe(200);
      const payments = (await response.json()) as PaymentListResponse;
      expect(payments).toEqual([]);
    });
  });
});
