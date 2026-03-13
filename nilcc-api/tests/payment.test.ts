import * as crypto from "node:crypto";
import { describe } from "vitest";
import { PathsV1 } from "#/common/paths";
import type { PaymentListResponse } from "#/payment/payment.dto";
import { PaymentService } from "#/payment/payment.service";
import { createTestFixtureExtension } from "./fixture/it";

describe("Payment", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  describe("PaymentService.computeCredits", () => {
    const service = new PaymentService();

    it("should compute credits for whole tokens", async ({ expect }) => {
      // 1 token = 10^6 base units = 1000 credits
      const credits = service.computeCredits(BigInt(10 ** 6));
      expect(credits).toBe(1000);
    });

    it("should compute credits for multiple tokens", async ({ expect }) => {
      // 5 tokens = 5000 credits
      const credits = service.computeCredits(BigInt(5) * BigInt(10 ** 6));
      expect(credits).toBe(5000);
    });

    it("should compute credits for fractional tokens", async ({ expect }) => {
      // 1.5 tokens = 1500 credits
      const oneAndHalf = BigInt(10 ** 6) + BigInt(10 ** 6) / BigInt(2);
      const credits = service.computeCredits(oneAndHalf);
      expect(credits).toBe(1500);
    });

    it("should compute credits down to 0.001 token", async ({ expect }) => {
      // 0.1 token = 100 credits
      const credits = service.computeCredits(BigInt(10 ** 5));
      expect(credits).toBe(100);
    });

    it("should return 0 for sub-credit amounts", async ({ expect }) => {
      // Less than 0.001 token
      const credits = service.computeCredits(BigInt(10 ** 2));
      expect(credits).toBe(0);
    });

    it("should return 0 for zero amount", async ({ expect }) => {
      const credits = service.computeCredits(BigInt(0));
      expect(credits).toBe(0);
    });
  });

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
          credits: 0,
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
      expect(payment?.creditedAmount).toBe(2000); // 2 tokens = 2000 credits

      // Verify account credits were updated
      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.credits).toBe(2000);
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
          credits: 0,
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

      // Credits should only be applied once (1 token = 1000 credits)
      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.credits).toBe(1000);
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
          credits: 0,
        })
        .submit();

      const result = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 4000,
        fromAddress: walletAddress,
        amount: BigInt(10 ** 5), // 0.1 token = 100 credits
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(result).not.toBeNull();
      expect(result?.creditedAmount).toBe(100);

      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.credits).toBe(100);
    });

    it("should return null for zero-credit amount", async ({
      expect,
      bindings,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      await clients.admin
        .createAccount({
          name: "zero-credit-test",
          walletAddress,
          credits: 0,
        })
        .submit();

      const result = await bindings.services.payment.processEvent(bindings, {
        txHash: `0x${crypto.randomBytes(32).toString("hex")}`,
        logIndex: 0,
        blockNumber: 4001,
        fromAddress: walletAddress,
        amount: BigInt(10 ** 2), // 0.0001 token, below 1 credit
        digest: `0x${crypto.randomBytes(32).toString("hex")}`,
      });

      expect(result).toBeNull();
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
          credits: 0,
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
          credits: 0,
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
      expect(payments[0].creditedAmount).toBe(3000); // 3 tokens = 3000 credits
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
          credits: 0,
        })
        .submit();
      const account2 = await clients.admin
        .createAccount({
          name: "pay-other",
          walletAddress: wallet2,
          credits: 0,
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
