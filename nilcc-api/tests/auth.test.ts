import * as crypto from "node:crypto";
import { generatePrivateKey, privateKeyToAccount } from "viem/accounts";
import { describe } from "vitest";
import type { ChallengeResponse, LoginResponse } from "#/auth/auth.dto";
import { PathsV1 } from "#/common/paths";
import { createTestFixtureExtension } from "./fixture/it";

type ErrorResponseBody = {
  kind: string;
};

type MeResponseBody = {
  walletAddress: string;
};

describe("Auth", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  describe("challenge", () => {
    it("should return a challenge message for a valid wallet address", async ({
      expect,
      app,
    }) => {
      const account = privateKeyToAccount(generatePrivateKey());
      const response = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });

      expect(response.status).toBe(200);
      const body = (await response.json()) as ChallengeResponse;
      expect(body.message).toContain("Sign in to nilCC");
      expect(body.message).toContain(account.address);
      expect(body.nonce).toBeDefined();
      expect(body.nonce).toMatch(
        /^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/,
      );
    });

    it("should reject an invalid wallet address format", async ({
      expect,
      app,
    }) => {
      const response = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: "not-an-address" }),
      });

      expect(response.status).toBe(400);
    });

    it("should reject a missing wallet address", async ({ expect, app }) => {
      const response = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({}),
      });

      expect(response.status).toBe(400);
    });
  });

  describe("login", () => {
    it("should authenticate with a valid signed challenge", async ({
      expect,
      app,
    }) => {
      const account = privateKeyToAccount(generatePrivateKey());

      // Step 1: Get a challenge
      const challengeResponse = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });
      const challenge = (await challengeResponse.json()) as ChallengeResponse;

      // Step 2: Sign the message
      const signature = await account.signMessage({
        message: challenge.message,
      });

      // Step 3: Login
      const loginResponse = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
          message: challenge.message,
          signature,
        }),
      });

      expect(loginResponse.status).toBe(200);
      const loginBody = (await loginResponse.json()) as LoginResponse;
      expect(loginBody.token).toBeDefined();
      expect(loginBody.expiresAt).toBeDefined();
      expect(loginBody.account).toBeDefined();
      expect(loginBody.account.walletAddress).toBe(
        account.address.toLowerCase(),
      );
      expect(loginBody.account.credits).toBe(0);
      expect(loginBody.account.accountId).toBeDefined();
    });

    it("should auto-create an account on first sign-in", async ({
      expect,
      app,
      bindings,
    }) => {
      const account = privateKeyToAccount(generatePrivateKey());

      // Verify no account exists
      const existingAccount =
        await bindings.services.account.findByWalletAddress(
          bindings,
          account.address.toLowerCase(),
        );
      expect(existingAccount).toBeNull();

      // Do the full SIWE login flow
      const challengeResponse = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });
      const challenge = (await challengeResponse.json()) as ChallengeResponse;
      const signature = await account.signMessage({
        message: challenge.message,
      });
      const loginResponse = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: challenge.message, signature }),
      });

      expect(loginResponse.status).toBe(200);

      // Verify account was created
      const createdAccount =
        await bindings.services.account.findByWalletAddress(
          bindings,
          account.address.toLowerCase(),
        );
      expect(createdAccount).not.toBeNull();
      expect(createdAccount?.credits).toBe(0);
    });

    it("should reject an invalid signature", async ({ expect, app }) => {
      const account = privateKeyToAccount(generatePrivateKey());
      const otherAccount = privateKeyToAccount(generatePrivateKey());

      // Get challenge for one wallet
      const challengeResponse = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });
      const challenge = (await challengeResponse.json()) as ChallengeResponse;

      // Sign with a different wallet
      const signature = await otherAccount.signMessage({
        message: challenge.message,
      });

      const loginResponse = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: challenge.message, signature }),
      });

      expect(loginResponse.status).toBe(401);
      const body = (await loginResponse.json()) as ErrorResponseBody;
      expect(body.kind).toBe("AUTHENTICATION_FAILED");
    });

    it("should reject a reused nonce", async ({ expect, app }) => {
      const account = privateKeyToAccount(generatePrivateKey());

      // Get challenge and login
      const challengeResponse = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });
      const challenge = (await challengeResponse.json()) as ChallengeResponse;
      const signature = await account.signMessage({
        message: challenge.message,
      });

      // First login should succeed
      const firstLogin = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: challenge.message, signature }),
      });
      expect(firstLogin.status).toBe(200);

      // Second login with same challenge should fail
      const secondLogin = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: challenge.message, signature }),
      });
      expect(secondLogin.status).toBe(401);
      const body = (await secondLogin.json()) as ErrorResponseBody;
      expect(body.kind).toBe("AUTHENTICATION_FAILED");
    });

    it("should reject an invalid message format", async ({ expect, app }) => {
      const account = privateKeyToAccount(generatePrivateKey());
      const badMessage = "this is not a valid challenge message";
      const signature = await account.signMessage({ message: badMessage });

      const loginResponse = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: badMessage, signature }),
      });

      expect(loginResponse.status).toBe(401);
    });

    it("should reject a missing signature", async ({ expect, app }) => {
      const response = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: "some message" }),
      });

      expect(response.status).toBe(400);
    });
  });

  describe("JWT authentication", () => {
    it("should allow accessing protected endpoints with a valid JWT", async ({
      expect,
      app,
    }) => {
      const account = privateKeyToAccount(generatePrivateKey());

      // Complete SIWE login flow
      const challengeResponse = await app.request(PathsV1.auth.challenge, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ walletAddress: account.address }),
      });
      const challenge = (await challengeResponse.json()) as ChallengeResponse;
      const signature = await account.signMessage({
        message: challenge.message,
      });
      const loginResponse = await app.request(PathsV1.auth.login, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ message: challenge.message, signature }),
      });
      const { token } = (await loginResponse.json()) as LoginResponse;

      // Use the JWT to access a protected endpoint
      const meResponse = await app.request(PathsV1.account.me, {
        method: "GET",
        headers: { authorization: `Bearer ${token}` },
      });

      expect(meResponse.status).toBe(200);
      const me = (await meResponse.json()) as MeResponseBody;
      expect(me.walletAddress).toBe(account.address.toLowerCase());
    });

    it("should reject requests with an invalid JWT", async ({
      expect,
      app,
    }) => {
      const response = await app.request(PathsV1.account.me, {
        method: "GET",
        headers: { authorization: "Bearer invalid.jwt.token" },
      });

      expect(response.status).toBe(401);
    });

    it("should reject invalid JWT-like bearer token on mixed-auth routes", async ({
      expect,
      app,
    }) => {
      const response = await app.request(PathsV1.workloadTiers.list, {
        method: "GET",
        headers: { authorization: "Bearer invalid.jwt.token" },
      });

      expect(response.status).toBe(401);
    });

    it("should reject requests with no authorization", async ({
      expect,
      app,
    }) => {
      const response = await app.request(PathsV1.account.me, {
        method: "GET",
      });

      expect(response.status).toBe(401);
    });

    it("should reject JWT via x-api-key header", async ({
      expect,
      app,
      issueJwt,
      clients,
    }) => {
      // Create an account and get a JWT
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({ name: "jwt-via-apikey", walletAddress, credits: 50 })
        .submit();

      const jwt = await issueJwt(account.accountId, walletAddress);

      // JWT is only accepted via Authorization: Bearer now.
      const response = await app.request(PathsV1.account.me, {
        method: "GET",
        headers: { "x-api-key": jwt },
      });

      expect(response.status).toBe(401);
    });

    it("should allow valid JWT even when x-api-key header is invalid", async ({
      expect,
      app,
      issueJwt,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({
          name: "jwt-with-garbage-apikey",
          walletAddress,
          credits: 1,
        })
        .submit();
      const jwt = await issueJwt(account.accountId, walletAddress);

      const response = await app.request(PathsV1.account.me, {
        method: "GET",
        headers: {
          authorization: `Bearer ${jwt}`,
          "x-api-key": "not-a-valid-admin-key",
        },
      });

      expect(response.status).toBe(200);
    });

    it("should prioritize global admin x-api-key over invalid bearer token", async ({
      expect,
      app,
      bindings,
    }) => {
      const response = await app.request(PathsV1.account.list, {
        method: "GET",
        headers: {
          "x-api-key": bindings.config.adminApiKey,
          authorization: "Bearer invalid.jwt.token",
        },
      });

      expect(response.status).toBe(200);
    });

    it("should reject malformed authorization schemes", async ({
      expect,
      app,
      issueJwt,
      clients,
    }) => {
      const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
      const account = await clients.admin
        .createAccount({ name: "malformed-auth", walletAddress, credits: 1 })
        .submit();
      const jwt = await issueJwt(account.accountId, walletAddress);

      const response = await app.request(PathsV1.account.me, {
        method: "GET",
        headers: {
          authorization: `Token ${jwt}`,
        },
      });

      expect(response.status).toBe(401);
    });
  });
});
