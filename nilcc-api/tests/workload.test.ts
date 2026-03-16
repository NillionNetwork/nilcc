import * as crypto from "node:crypto";
import { describe } from "vitest";
import { MINIMUM_SPENDABLE_BALANCE_NIL, usdToNil } from "#/common/nil";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
import type {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
} from "#/workload/workload.dto";
import type { MockNilPriceService, MockTimeService } from "./fixture/fixture";
import { createTestFixtureExtension } from "./fixture/it";
import { UserClient } from "./fixture/test-client";

describe("workload CRUD", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});
  let myWorkload: null | CreateWorkloadResponse = null;

  const createWorkloadRequest: CreateWorkloadRequest = {
    name: "my-cool-workload",
    artifactsVersion: "aaa",
    dockerCompose: `
services:
  app:
    image: nginx
    ports:
      - '80'
`,
    envVars: {
      MY_SECRET: "42",
    },
    files: {
      "foo_-choop/bar42_beep.txt": "aGkgbW9t",
    },
    dockerCredentials: [
      { server: "registry.example.com", username: "foo", password: "bar" },
    ],
    publicContainerName: "app",
    publicContainerPort: 80,
    memory: 1024,
    cpus: 1,
    disk: 10,
    gpus: 0,
    heartbeat: {
      measurementHashUrl: "https://foo.com/potato",
    },
  };

  const myMetalInstance: RegisterMetalInstanceRequest = {
    metalInstanceId: "c92c86e4-c7e5-4bb3-a5f5-45945b5593e4",
    agentVersion: "v0.1.0",
    publicIp: "127.0.0.1",
    token: "mock-agent-token",
    hostname: "my-metal-instance",
    memoryMb: {
      total: 8192,
      reserved: 2048,
    },
    cpus: {
      total: 8,
      reserved: 2,
    },
    diskSpaceGb: {
      total: 1024,
      reserved: 128,
    },
    gpus: 0,
  };

  it("should fail to create a workload if there isn't a metal instance", async ({
    expect,
    clients,
  }) => {
    await clients.admin.enableArtifactVersion("aaa").submit();
    await clients.admin
      .createTier({
        name: "tiny",
        cost: 1,
        cpus: 1,
        gpus: 0,
        memoryMb: 1024,
        diskGb: 10,
      })
      .submit();
    const status = await clients.user
      .createWorkload(createWorkloadRequest)
      .status();
    expect(status).toBe(503);
  });

  it("should create a workload", async ({ expect, clients }) => {
    // Register the agent and heartbeat to indicate which artifact versions it supports.
    await clients.metalInstance.register(myMetalInstance).submit();
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId, ["aaa"])
      .submit();

    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();
    expect(workload.name).equals(createWorkloadRequest.name);
    expect(workload.domain).equals(
      `${workload.workloadId}.workloads.public.localhost`,
    );
    expect(workload.metalInstanceDomain).equals(
      `${myMetalInstance.metalInstanceId}.agents.private.localhost`,
    );
    expect(workload.usdCostPerMin).toBe(1);
    // store it for other tests to re-use it
    myWorkload = workload;
  });

  it("should not be allowed to create a workload without a matching tier", async ({
    expect,
    clients,
  }) => {
    const request = { ...createWorkloadRequest, cpus: 5, memory: 13 };
    expect(await clients.user.createWorkload(request).status()).toBe(400);
  });

  it("should fail to create a workload if it doesn't fit in the metal instance", async ({
    expect,
    clients,
  }) => {
    await clients.admin
      .createTier({
        name: "not so tiny",
        cost: 2,
        cpus: 63,
        gpus: 0,
        memoryMb: 1024,
        diskGb: 10,
      })
      .submit();
    const overloadedWorkloadRequest = {
      ...createWorkloadRequest,
      cpus: 63, // Exceeding the available CPU
    };
    const status = await clients.user
      .createWorkload(overloadedWorkloadRequest)
      .status();
    expect(status).equal(503);
  });

  it("should get a workload", async ({ expect, clients }) => {
    const workload = await clients.user
      .getWorkload(myWorkload!.workloadId)
      .submit();
    expect(workload.name).equals(myWorkload!.name);
  });

  it("should list the workloads", async ({ expect, clients }) => {
    const workloads = await clients.user.listWorkloads().submit();
    expect(workloads.length).greaterThan(0);
    expect(workloads[0].name).equals(myWorkload!.name);
  });

  it("should delete a workload", async ({ expect, clients }) => {
    await clients.user.deleteWorkload(myWorkload!.workloadId).submit();

    // Verify deletion
    const status = await clients.user
      .getWorkload(myWorkload!.workloadId)
      .status();
    expect(status).equal(404);
  });

  it("should update a workload's state", async ({ expect, clients }) => {
    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();

    const timestamp = "2025-09-02T20:46:44.666Z";
    await clients.metalInstance
      .submitEvent({
        metalInstanceId: myMetalInstance.metalInstanceId,
        workloadId: workload.workloadId,
        event: { kind: "starting" },
        timestamp,
      })
      .submit();

    const updatedWorkload = await clients.user
      .getWorkload(workload!.workloadId)
      .submit();
    expect(updatedWorkload.status).toBe("starting");

    const eventsBody = await clients.user
      .listEvents(workload!.workloadId)
      .submit();
    const events = eventsBody.events;
    events.sort(
      (a, b) =>
        new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
    );

    expect(events).toHaveLength(2);
    const eventKinds = events.map((e) => e.details.kind);
    expect(eventKinds).toEqual(["created", "starting"]);

    const lastEvent = events[events.length - 1];
    expect(lastEvent.timestamp).toEqual(timestamp);
  });

  it("submit a warning event", async ({ expect, clients }) => {
    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();

    const timestamp = "2025-09-02T20:46:44.666Z";
    await clients.metalInstance
      .submitEvent({
        metalInstanceId: myMetalInstance.metalInstanceId,
        workloadId: workload.workloadId,
        event: { kind: "warning", message: "hello" },
        timestamp,
      })
      .submit();

    const eventsBody = await clients.user
      .listEvents(workload!.workloadId)
      .submit();
    const events = eventsBody.events;
    events.sort(
      (a, b) =>
        new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime(),
    );

    expect(events).toHaveLength(2);
    const warning = events[1];
    expect(warning.details).toEqual({ kind: "warning", message: "hello" });

    await clients.user.deleteWorkload(workload.workloadId).submit();
  });

  it("should allow creating a workload using a custom domain", async ({
    expect,
    clients,
  }) => {
    const createWorkloadRequest: CreateWorkloadRequest = {
      name: "some",
      artifactsVersion: "aaa",
      dockerCompose: `
services:
  app:
    image: nginx
    ports:
      - '80'
`,
      domain: "foo.com",
      publicContainerName: "app",
      publicContainerPort: 80,
      memory: 1024,
      cpus: 1,
      disk: 10,
      gpus: 0,
    };
    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();
    expect(workload.domain).toBe("foo.com");
    await clients.user.deleteWorkload(workload.workloadId).submit();
  });

  it("should now allow cross account operations", async ({
    expect,
    app,
    bindings,
    clients,
    issueJwt,
  }) => {
    const walletAddress = `0x${Buffer.from(crypto.getRandomValues(new Uint8Array(20))).toString("hex")}`;
    const account = await clients.admin
      .createAccount({ name: "cross-account", walletAddress, balance: 0 })
      .submit();
    const jwt = await issueJwt(account.accountId, account.walletAddress);
    const client = new UserClient({
      app,
      bindings,
      apiToken: jwt,
    });
    // Make sure there's something with the regular client, just in case the above tests are changed somehow.
    const workloads = await clients.user.listWorkloads().submit();
    expect(workloads).toHaveLength(1);

    const workload = workloads[0];
    expect(await client.listWorkloads().submit()).toHaveLength(0);
    expect(await client.getWorkload(workload.workloadId).status()).toBe(401);
    expect(await client.deleteWorkload(workload.workloadId).status()).toBe(401);
    expect(await client.listEvents(workload.workloadId).status()).toBe(401);
    expect(await client.listContainers(workload.workloadId).status()).toBe(401);
    expect(
      await client.containerLogs(workload.workloadId, "foo").status(),
    ).toBe(401);
    expect(await client.logs(workload.workloadId).status()).toBe(401);
  });

  it("should deny cross-account workload access between two workload owners", async ({
    expect,
    app,
    bindings,
    clients,
    issueJwt,
  }) => {
    const walletA = `0x${crypto.randomBytes(20).toString("hex")}`;
    const accountA = await clients.admin
      .createAccount({
        name: "owner-a",
        walletAddress: walletA,
        balance: 100000,
      })
      .submit();
    const jwtA = await issueJwt(accountA.accountId, accountA.walletAddress);
    const ownerA = new UserClient({
      app,
      bindings,
      apiToken: jwtA,
    });

    const walletB = `0x${crypto.randomBytes(20).toString("hex")}`;
    const accountB = await clients.admin
      .createAccount({
        name: "owner-b",
        walletAddress: walletB,
        balance: 100000,
      })
      .submit();
    const jwtB = await issueJwt(accountB.accountId, accountB.walletAddress);
    const ownerB = new UserClient({
      app,
      bindings,
      apiToken: jwtB,
    });

    const workloadA = await ownerA
      .createWorkload({
        ...createWorkloadRequest,
        name: "owner-a-workload",
      })
      .submit();
    const workloadB = await ownerB
      .createWorkload({
        ...createWorkloadRequest,
        name: "owner-b-workload",
      })
      .submit();

    const listA = await ownerA.listWorkloads().submit();
    const listB = await ownerB.listWorkloads().submit();
    expect(listA.map((w) => w.workloadId)).toContain(workloadA.workloadId);
    expect(listA.map((w) => w.workloadId)).not.toContain(workloadB.workloadId);
    expect(listB.map((w) => w.workloadId)).toContain(workloadB.workloadId);
    expect(listB.map((w) => w.workloadId)).not.toContain(workloadA.workloadId);

    expect(await ownerB.getWorkload(workloadA.workloadId).status()).toBe(401);
    expect(await ownerB.deleteWorkload(workloadA.workloadId).status()).toBe(
      401,
    );
    expect(await ownerB.restartWorkload(workloadA.workloadId).status()).toBe(
      401,
    );
    expect(await ownerB.listEvents(workloadA.workloadId).status()).toBe(401);
    expect(await ownerB.listContainers(workloadA.workloadId).status()).toBe(
      401,
    );
    expect(
      await ownerB.containerLogs(workloadA.workloadId, "app").status(),
    ).toBe(401);
    expect(await ownerB.logs(workloadA.workloadId).status()).toBe(401);

    expect(await ownerA.getWorkload(workloadB.workloadId).status()).toBe(401);
    expect(await ownerA.deleteWorkload(workloadB.workloadId).status()).toBe(
      401,
    );
    expect(await ownerA.restartWorkload(workloadB.workloadId).status()).toBe(
      401,
    );
    expect(await ownerA.listEvents(workloadB.workloadId).status()).toBe(401);
    expect(await ownerA.listContainers(workloadB.workloadId).status()).toBe(
      401,
    );
    expect(
      await ownerA.containerLogs(workloadB.workloadId, "app").status(),
    ).toBe(401);
    expect(await ownerA.logs(workloadB.workloadId).status()).toBe(401);

    await ownerA.deleteWorkload(workloadA.workloadId).submit();
    await ownerB.deleteWorkload(workloadB.workloadId).submit();
  });

  it("should allow performing workload actions", async ({
    expect,
    clients,
  }) => {
    const workloads = await clients.user.listWorkloads().submit();
    const workload = workloads[0];
    const workloadId = workload.workloadId;

    // restart with env vars
    {
      await clients.user.restartWorkload(workloadId, { foo: "42" }).submit();
      const workload = await clients.user.getWorkload(workloadId).submit();
      expect(workload.envVars).toEqual({ foo: "42" });
    }

    // restart without env vars, nothing should have changed
    {
      await clients.user.restartWorkload(workloadId).submit();
      const workload = await clients.user.getWorkload(workloadId).submit();
      expect(workload.envVars).toEqual({ foo: "42" });
    }

    // restart by resetting env vars to an empty set
    {
      await clients.user.restartWorkload(workloadId, {}).submit();
      const workload = await clients.user.getWorkload(workloadId).submit();
      expect(workload.envVars).toEqual({});
    }

    await clients.user.deleteWorkload(workloadId).submit();
  });

  it("should not allow overcommitting balance", async ({ expect, clients }) => {
    const workloads = await clients.user.listWorkloads().submit();
    const totalUsage = workloads
      .map((w) => w.usdCostPerMin)
      .reduce((a, b) => a + b, 0);

    const account = await clients.user.myAccount().submit();
    const burnRate = account.burnRatePerMin;
    const expectedBurnRate = usdToNil(totalUsage, 1.0);
    expect(burnRate).toBe(expectedBurnRate);

    // Compute how much is the maximum USD cost we can spend per minute
    // balance is now in decimal NIL; at nilPrice=1.0, NIL=USD
    // We need: (totalUsage + newCost) * 5 minutes <= balance
    const maxCredits = Math.floor(account.balance / 5 - totalUsage);
    // Create 2 tiers: one with that value + 1 (too expensive), and another one with that value
    await clients.admin
      .createTier({
        name: "too-expensive",
        cost: maxCredits + 1,
        cpus: 1,
        gpus: 0,
        memoryMb: 1024,
        diskGb: 11,
      })
      .submit();
    await clients.admin
      .createTier({
        name: "not-too-expensive",
        cost: maxCredits,
        cpus: 1,
        gpus: 0,
        memoryMb: 1024,
        diskGb: 12,
      })
      .submit();

    // The too expensive one should fail
    expect(
      await clients.user
        .createWorkload({
          ...createWorkloadRequest,
          cpus: 1,
          gpus: 0,
          memory: 1024,
          disk: 11,
        })
        .status(),
    ).toBe(412);
    // The other one should not
    expect(
      await clients.user
        .createWorkload({
          ...createWorkloadRequest,
          cpus: 1,
          gpus: 0,
          memory: 1024,
          disk: 12,
        })
        .status(),
    ).toBe(200);
  });

  it("should subtract balance on heartbeat", async ({
    app,
    bindings,
    expect,
    clients,
    issueJwt,
  }) => {
    const heartbeatWallet = `0x${Buffer.from(crypto.getRandomValues(new Uint8Array(20))).toString("hex")}`;
    let otherAccount = await clients.admin
      .createAccount({
        name: "heartbeat-account",
        walletAddress: heartbeatWallet,
        balance: 15000,
      })
      .submit();
    const heartbeatJwt = await issueJwt(
      otherAccount.accountId,
      otherAccount.walletAddress,
    );
    const client = new UserClient({
      app,
      bindings,
      apiToken: heartbeatJwt,
    });
    await client.createWorkload(createWorkloadRequest).submit();

    const workloads = await clients.user.listWorkloads().submit();
    const usdCostPerMin = workloads
      .map((w) => w.usdCostPerMin)
      .reduce((a, b) => a + b, 0);
    const account = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    const timeService = bindings.services.time as MockTimeService;
    timeService.advance(61);

    // Heartbeat once, this should subtract the NIL cost.
    // With nilPrice=1.0, usdCostPerMin=1 => nilCost = 1 NIL per minute
    const nilCostPerMin = usdToNil(usdCostPerMin, 1.0);
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId, [])
      .submit();
    const updatedAccount = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    expect(updatedAccount.balance).toBe(account.balance - nilCostPerMin);

    // Heartbeat again, this shouldn't do anything.
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId, [])
      .submit();
    const latestAccount = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    expect(latestAccount.balance).toBe(updatedAccount.balance);

    otherAccount = await clients.admin
      .getAccount(otherAccount.accountId)
      .submit();
    // should be the original minus 1 minute of NIL cost for the one workload we're running
    // With nilPrice=1.0 and cost=1 USD/min, that's 1 NIL
    const expectedBalance = 15000 - usdToNil(1, 1.0);
    expect(otherAccount.balance).toBe(expectedBalance);
  });

  it("should stop workloads when heartbeat leaves less than one cent of NIL", async ({
    app,
    bindings,
    expect,
    clients,
    issueJwt,
  }) => {
    const nilPrice = bindings.services.nilPrice as MockNilPriceService;
    nilPrice.setPrice(3.0);
    const secondMetalInstance: RegisterMetalInstanceRequest = {
      ...myMetalInstance,
      metalInstanceId: "f42c86e4-c7e5-4bb3-a5f5-45945b5593e4",
      hostname: "my-second-metal-instance",
    };
    await clients.metalInstance.register(secondMetalInstance).submit();
    await clients.metalInstance
      .heartbeat(secondMetalInstance.metalInstanceId, ["aaa"])
      .submit();

    const heartbeatWallet = `0x${Buffer.from(crypto.getRandomValues(new Uint8Array(20))).toString("hex")}`;
    const usdCostPerMin = 0.2;
    const minuteNilCost = usdToNil(usdCostPerMin, 3.0);
    const startingBalance = minuteNilCost * 5 + 1e-16;
    expect(startingBalance - minuteNilCost * 5).toBeLessThan(
      MINIMUM_SPENDABLE_BALANCE_NIL,
    );
    await clients.admin
      .createTier({
        name: "sub-cent-heartbeat-tier",
        cost: usdCostPerMin,
        cpus: 1,
        gpus: 0,
        memoryMb: 1024,
        diskGb: 13,
      })
      .submit();
    const account = await clients.admin
      .createAccount({
        name: "sub-cent-heartbeat-account",
        walletAddress: heartbeatWallet,
        balance: startingBalance,
      })
      .submit();
    const heartbeatJwt = await issueJwt(
      account.accountId,
      account.walletAddress,
    );
    const client = new UserClient({
      app,
      bindings,
      apiToken: heartbeatJwt,
    });
    await client
      .createWorkload({
        ...createWorkloadRequest,
        disk: 13,
      })
      .submit();

    const timeService = bindings.services.time as MockTimeService;
    for (let i = 0; i < 5; i++) {
      timeService.advance(61);
      await clients.metalInstance
        .heartbeat(secondMetalInstance.metalInstanceId, [])
        .submit();
    }

    const updatedAccount = await clients.admin
      .getAccount(account.accountId)
      .submit();
    expect(updatedAccount.balance).toBe(0);
    expect(updatedAccount.balance).toBeLessThan(MINIMUM_SPENDABLE_BALANCE_NIL);
  });
});
