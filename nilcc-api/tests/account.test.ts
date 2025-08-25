import { describe } from "vitest";
import type { CreateAccountRequest } from "#/account/account.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("Account", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("should create an account that hasn't been created", async ({
    expect,
    clients,
  }) => {
    const request: CreateAccountRequest = {
      name: "my favorite account",
      credits: 100,
    };
    const account = await clients.admin.createAccount(request).submit();
    expect(account.name).toBe(request.name);
    expect(account.credits).toBe(request.credits);

    // Creating it again should fail
    expect(await clients.admin.createAccount(request).status()).toBe(409);
  });

  it("should allow listing", async ({ expect, clients }) => {
    const account = await clients.admin
      .createAccount({ name: "foo", credits: 1 })
      .submit();
    const accounts = await clients.admin.listAccounts().submit();
    expect(accounts).toContainEqual(account);
  });

  it("should allow lookups", async ({ expect, clients }) => {
    const account = await clients.admin
      .createAccount({ name: "bar", credits: 2 })
      .submit();
    const details = await clients.admin.getAccount(account.accountId).submit();
    expect(details).toEqual(account);
  });
});
