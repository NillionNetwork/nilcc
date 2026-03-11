import * as crypto from "node:crypto";
import { describe } from "vitest";
import { PathsV1 } from "#/common/paths";
import { createTestFixtureExtension } from "./fixture/it";

describe("API keys", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  it("supports CRUD from global admin", async ({ expect, clients }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "api-key-admin", walletAddress, credits: 10 })
      .submit();

    const created = await clients.admin
      .createApiKey({
        accountId: account.accountId,
        type: "user",
        active: true,
      })
      .submit();

    expect(created.accountId).toBe(account.accountId);
    expect(created.type).toBe("user");
    expect(created.active).toBe(true);

    const list = await clients.admin.listApiKeys(account.accountId).submit();
    expect(list.map((k) => k.id)).toContain(created.id);

    const updated = await clients.admin
      .updateApiKey({
        id: created.id,
        active: false,
        type: "account-admin",
      })
      .submit();
    expect(updated.active).toBe(false);
    expect(updated.type).toBe("account-admin");

    await clients.admin.deleteApiKey({ id: created.id }).submit();
    const listAfter = await clients.admin
      .listApiKeys(account.accountId)
      .submit();
    expect(listAfter.map((k) => k.id)).not.toContain(created.id);
  });

  it("allows account owner JWT to manage own api keys", async ({
    expect,
    app,
    issueJwt,
    clients,
  }) => {
    const walletAddress = `0x${crypto.randomBytes(20).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "jwt-owner", walletAddress, credits: 10 })
      .submit();
    const jwt = await issueJwt(account.accountId, walletAddress);

    const createResponse = await app.request(PathsV1.apiKeys.create, {
      method: "POST",
      headers: {
        authorization: `Bearer ${jwt}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        accountId: account.accountId,
        type: "user",
        active: true,
      }),
    });
    expect(createResponse.status).toBe(200);
    const created = (await createResponse.json()) as { id: string };

    const listResponse = await app.request(
      PathsV1.apiKeys.listByAccount.replace(":accountId", account.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${jwt}`,
        },
      },
    );
    expect(listResponse.status).toBe(200);

    const updateResponse = await app.request(PathsV1.apiKeys.update, {
      method: "PUT",
      headers: {
        authorization: `Bearer ${jwt}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ id: created.id, active: false }),
    });
    expect(updateResponse.status).toBe(200);

    const deleteResponse = await app.request(PathsV1.apiKeys.delete, {
      method: "POST",
      headers: {
        authorization: `Bearer ${jwt}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ id: created.id }),
    });
    expect(deleteResponse.status).toBe(200);
  });

  it("allows account-admin api key on own identity operations", async ({
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

    const listResponse = await app.request(
      PathsV1.apiKeys.listByAccount.replace(":accountId", me.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${accountAdminKey.id}`,
        },
      },
    );
    expect(listResponse.status).toBe(200);

    const accountResponse = await app.request(
      PathsV1.account.read.replace(":id", me.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${accountAdminKey.id}`,
        },
      },
    );
    expect(accountResponse.status).toBe(200);
  });

  it("denies user api key on identity management routes", async ({
    expect,
    clients,
    app,
  }) => {
    const me = await clients.user.myAccount().submit();
    const userKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "user",
        active: true,
      })
      .submit();

    const accountReadResponse = await app.request(
      PathsV1.account.read.replace(":id", me.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${userKey.id}`,
        },
      },
    );
    expect(accountReadResponse.status).toBe(401);

    const meResponse = await app.request(PathsV1.account.me, {
      method: "GET",
      headers: {
        authorization: `Bearer ${userKey.id}`,
      },
    });
    expect(meResponse.status).toBe(401);

    const keyListResponse = await app.request(
      PathsV1.apiKeys.listByAccount.replace(":accountId", me.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${userKey.id}`,
        },
      },
    );
    expect(keyListResponse.status).toBe(401);
  });

  it("allows user api key on non-identity routes and blocks inactive keys", async ({
    expect,
    clients,
    app,
  }) => {
    const me = await clients.user.myAccount().submit();
    const userKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "user",
        active: true,
      })
      .submit();

    const tierResponse = await app.request(PathsV1.workloadTiers.list, {
      method: "GET",
      headers: {
        authorization: `Bearer ${userKey.id}`,
      },
    });
    expect(tierResponse.status).toBe(200);

    await clients.admin
      .updateApiKey({ id: userKey.id, active: false })
      .submit();

    const tierInactiveResponse = await app.request(PathsV1.workloadTiers.list, {
      method: "GET",
      headers: {
        authorization: `Bearer ${userKey.id}`,
      },
    });
    expect(tierInactiveResponse.status).toBe(401);
  });

  it("denies account-admin api key from other accounts", async ({
    expect,
    clients,
    app,
  }) => {
    const sourceWallet = `0x${crypto.randomBytes(20).toString("hex")}`;
    const source = await clients.admin
      .createAccount({
        name: "source",
        walletAddress: sourceWallet,
        credits: 1,
      })
      .submit();
    const targetWallet = `0x${crypto.randomBytes(20).toString("hex")}`;
    const target = await clients.admin
      .createAccount({
        name: "target",
        walletAddress: targetWallet,
        credits: 1,
      })
      .submit();

    const accountAdminKey = await clients.admin
      .createApiKey({
        accountId: source.accountId,
        type: "account-admin",
        active: true,
      })
      .submit();

    const response = await app.request(
      PathsV1.apiKeys.listByAccount.replace(":accountId", target.accountId),
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${accountAdminKey.id}`,
        },
      },
    );

    expect(response.status).toBe(401);
  });

  it("denies account-admin api key on global admin account routes", async ({
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

    const listResponse = await app.request(PathsV1.account.list, {
      method: "GET",
      headers: {
        authorization: `Bearer ${accountAdminKey.id}`,
      },
    });
    expect(listResponse.status).toBe(401);

    const createResponse = await app.request(PathsV1.account.create, {
      method: "POST",
      headers: {
        authorization: `Bearer ${accountAdminKey.id}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        name: "not-allowed",
        walletAddress: `0x${crypto.randomBytes(20).toString("hex")}`,
        credits: 0,
      }),
    });
    expect(createResponse.status).toBe(401);
  });

  it("invalidates api key authentication after delete", async ({
    expect,
    clients,
    app,
  }) => {
    const me = await clients.user.myAccount().submit();
    const userKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "user",
        active: true,
      })
      .submit();

    await clients.admin.deleteApiKey({ id: userKey.id }).submit();

    const response = await app.request(PathsV1.workloadTiers.list, {
      method: "GET",
      headers: {
        authorization: `Bearer ${userKey.id}`,
      },
    });
    expect(response.status).toBe(401);
  });

  it("rejects update payloads with no mutable fields", async ({
    expect,
    clients,
    app,
  }) => {
    const me = await clients.user.myAccount().submit();
    const userKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "user",
        active: true,
      })
      .submit();

    const response = await app.request(PathsV1.apiKeys.update, {
      method: "PUT",
      headers: {
        authorization: `Bearer ${clients.user._options.apiToken}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({ id: userKey.id }),
    });
    expect(response.status).toBe(400);
  });

  it("denies account-owner JWT from modifying other account keys", async ({
    expect,
    clients,
    app,
    issueJwt,
  }) => {
    const sourceWallet = `0x${crypto.randomBytes(20).toString("hex")}`;
    const source = await clients.admin
      .createAccount({
        name: "source-jwt",
        walletAddress: sourceWallet,
        credits: 1,
      })
      .submit();
    const sourceJwt = await issueJwt(source.accountId, sourceWallet);

    const targetWallet = `0x${crypto.randomBytes(20).toString("hex")}`;
    const target = await clients.admin
      .createAccount({
        name: "target-jwt",
        walletAddress: targetWallet,
        credits: 1,
      })
      .submit();
    const targetKey = await clients.admin
      .createApiKey({
        accountId: target.accountId,
        type: "user",
        active: true,
      })
      .submit();

    const updateResponse = await app.request(PathsV1.apiKeys.update, {
      method: "PUT",
      headers: {
        authorization: `Bearer ${sourceJwt}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        id: targetKey.id,
        active: false,
      }),
    });
    expect(updateResponse.status).toBe(401);

    const deleteResponse = await app.request(PathsV1.apiKeys.delete, {
      method: "POST",
      headers: {
        authorization: `Bearer ${sourceJwt}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify({
        id: targetKey.id,
      }),
    });
    expect(deleteResponse.status).toBe(401);
  });

  it("enforces workload route access by api key type", async ({
    expect,
    clients,
    app,
    bindings,
  }) => {
    const me = await clients.user.myAccount().submit();
    const userKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "user",
        active: true,
      })
      .submit();
    const accountAdminKey = await clients.admin
      .createApiKey({
        accountId: me.accountId,
        type: "account-admin",
        active: true,
      })
      .submit();

    const requestCreate = (authorization: string) =>
      app.request(PathsV1.workload.create, {
        method: "POST",
        headers: {
          authorization,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({}),
      });
    const requestDelete = (authorization: string) =>
      app.request(PathsV1.workload.delete, {
        method: "POST",
        headers: {
          authorization,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({}),
      });
    const requestRestart = (authorization: string) =>
      app.request(PathsV1.workload.restart, {
        method: "POST",
        headers: {
          authorization,
          "Content-Type": "application/json",
        },
        body: JSON.stringify({}),
      });

    expect((await requestCreate(`Bearer ${userKey.id}`)).status).toBe(400);
    expect((await requestDelete(`Bearer ${userKey.id}`)).status).toBe(400);
    expect((await requestRestart(`Bearer ${userKey.id}`)).status).toBe(400);

    expect((await requestCreate(`Bearer ${accountAdminKey.id}`)).status).toBe(
      400,
    );
    expect((await requestDelete(`Bearer ${accountAdminKey.id}`)).status).toBe(
      400,
    );
    expect((await requestRestart(`Bearer ${accountAdminKey.id}`)).status).toBe(
      400,
    );

    expect(
      (
        await app.request(PathsV1.workload.create, {
          method: "POST",
          headers: {
            "x-api-key": bindings.config.adminApiKey,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({}),
        })
      ).status,
    ).toBe(401);
    expect(
      (
        await app.request(PathsV1.workload.delete, {
          method: "POST",
          headers: {
            "x-api-key": bindings.config.adminApiKey,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({}),
        })
      ).status,
    ).toBe(401);
    expect(
      (
        await app.request(PathsV1.workload.restart, {
          method: "POST",
          headers: {
            "x-api-key": bindings.config.adminApiKey,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({}),
        })
      ).status,
    ).toBe(401);
  });
});
