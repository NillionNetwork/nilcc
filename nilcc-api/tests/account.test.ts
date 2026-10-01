import * as crypto from "node:crypto";
import { describe } from "vitest";
import type { CreateAccountRequest } from "#/account/account.dto";
import { PathsV1 } from "#/common/paths";
import { createTestFixtureExtension } from "./fixture/it";

describe("Account", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("should create an account that hasn't been created", async ({
    expect,
    clients,
  }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const request: CreateAccountRequest = {
      name: "my favorite account",
      walletAddress,
      balance: 100,
    };
    const account = await clients.admin.createAccount(request).submit();
    expect(account.name).toBe(request.name);
    expect(account.balance).toBe(request.balance);
    expect(account.walletAddress).toBe(walletAddress.toLowerCase());

    // Creating it again should fail
    expect(await clients.admin.createAccount(request).status()).toBe(409);
  });

  it("should allow listing", async ({ expect, clients }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "foo", walletAddress, balance: 1 })
      .submit();
    const accounts = await clients.admin.listAccounts().submit();
    expect(accounts).toContainEqual(account);
  });

  it("should allow lookups", async ({ expect, clients }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "bar", walletAddress, balance: 2 })
      .submit();
    const details = await clients.admin.getAccount(account.accountId).submit();
    expect(details).toEqual(account);
  });

  it("should allow self lookups", async ({ expect, clients }) => {
    const me = await clients.user.myAccount().submit();
    const account = await clients.admin.getAccount(me.accountId).submit();

    const expected: Record<string, unknown> = { ...account };
    expected.burnRatePerMin = 0;
    expect(me).toEqual(expected);
  });

  it("should allow updating", async ({ expect, clients }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "some name", walletAddress, balance: 2 })
      .submit();
    await clients.admin
      .updateAccount({ accountId: account.accountId, name: "some other name" })
      .submit();
    const expected = { ...account, name: "some other name" };
    const updated = await clients.admin.getAccount(account.accountId).submit();
    expect(updated).toEqual(expected);
  });

  it("should allow admins to add balance", async ({ expect, clients }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "balance", walletAddress, balance: 10 })
      .submit();
    const updated = await clients.admin
      .addBalance({ accountId: account.accountId, balance: 5 })
      .submit();
    expect(updated.balance).toBe(15);
  });

  it("should not allow account owners to add balance", async ({
    expect,
    clients,
    app,
  }) => {
    const me = await clients.user.myAccount().submit();
    const accountAdminKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "account-admin",
        active: true,
      })
      .submit();
    const jwt = clients.user._options.apiToken;

    for (const token of [jwt, accountAdminKey.id]) {
      const response = await app.request(PathsV1.account.addBalance, {
        method: "POST",
        headers: {
          authorization: `Bearer ${token}`,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({ accountId: me.accountId, balance: 1000000 }),
      });
      expect(response.status).toBe(401);
    }

    const after = await clients.admin.getAccount(me.accountId).submit();
    expect(after.balance).toBe(me.balance);
  });
});
