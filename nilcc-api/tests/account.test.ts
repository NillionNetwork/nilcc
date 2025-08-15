import { describe } from "vitest";
import { createTestFixtureExtension } from "./fixture/it";

describe("Account", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("should create an account that hasn't been created", async ({
    expect,
    clients,
  }) => {
    const name = "my favorite account";
    const account = await clients.admin.createAccount(name).submit();
    expect(account.name).toBe(name);

    // Creating it again should fail
    expect(await clients.admin.createAccount(name).status()).toBe(409);
  });

  it("should allow listing", async ({ expect, clients }) => {
    const name = "another account";
    const account = await clients.admin.createAccount(name).submit();
    const accounts = await clients.admin.listAccounts().submit();
    expect(accounts).toContainEqual(account);
  });

  it("should allow lookups", async ({ expect, clients }) => {
    const name = "yet another account";
    const account = await clients.admin.createAccount(name).submit();
    const details = await clients.admin.getAccount(account.accountId).submit();
    expect(details).toEqual(account);
  });
});
