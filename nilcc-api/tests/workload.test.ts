import { describe } from "vitest";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
import type {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
} from "#/workload/workload.dto";
import type { MockTimeService } from "./fixture/fixture";
import { createTestFixtureExtension } from "./fixture/it";
import { UserClient } from "./fixture/test-client";

describe("workload CRUD", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});
  let myWorkload: null | CreateWorkloadResponse = null;

  const createWorkloadRequest: CreateWorkloadRequest = {
    name: "my-cool-workload",
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
  };

  const myMetalInstance: RegisterMetalInstanceRequest = {
    metalInstanceId: "c92c86e4-c7e5-4bb3-a5f5-45945b5593e4",
    agentVersion: "v0.1.0",
    publicIp: "127.0.0.1",
    token: "my_token",
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
    await clients.metalInstance.register(myMetalInstance).submit();

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
    expect(workload.creditRate).toBe(1);
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

  it("should allow creating a workload using a specific artifact version", async ({
    expect,
    clients,
  }) => {
    const artifactsVersion = "abc";
    await clients.admin.enableArtifactVersion(artifactsVersion).submit();
    const createWorkloadRequest: CreateWorkloadRequest = {
      name: "some",
      dockerCompose: `
services:
  app:
    image: nginx
    ports:
      - '80'
`,
      artifactsVersion,
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
    expect(workload.artifactsVersion).toBe(artifactsVersion);
    await clients.user.deleteWorkload(workload.workloadId).submit();
  });

  it("should now allow cross account operations", async ({
    expect,
    app,
    bindings,
    clients,
  }) => {
    const account = await clients.admin
      .createAccount({ name: "cross-account", credits: 0 })
      .submit();
    const client = new UserClient({
      app,
      bindings,
      apiToken: account.apiToken,
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

  it("should allow performing workload actions", async ({ clients }) => {
    const workloads = await clients.user.listWorkloads().submit();
    const workload = workloads[0];
    await clients.user.restartWorkload(workload.workloadId).submit();
    await clients.user.stopWorkload(workload.workloadId).submit();
    await clients.user.startWorkload(workload.workloadId).submit();
  });

  it("should not allow overcommitting credits", async ({ expect, clients }) => {
    const workloads = await clients.user.listWorkloads().submit();
    const totalUsage = workloads
      .map((w) => w.creditRate)
      .reduce((a, b) => a + b, 0);

    const account = await clients.user.myAccount().submit();
    expect(account.creditRate).toBe(totalUsage);

    // Compute how much is the maximum credits we can spend per minute
    const maxCredits = Math.floor((account.credits - totalUsage * 5) / 5);
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

  it("should subtract credits on heartbeat", async ({
    app,
    bindings,
    expect,
    clients,
  }) => {
    let otherAccount = await clients.admin
      .createAccount({ name: "heartbeat-account", credits: 1500 })
      .submit();
    const client = new UserClient({
      app,
      bindings,
      apiToken: otherAccount.apiToken,
    });
    await client.createWorkload(createWorkloadRequest).submit();

    const workloads = await clients.user.listWorkloads().submit();
    const creditRate = workloads
      .map((w) => w.creditRate)
      .reduce((a, b) => a + b, 0);
    const account = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    const timeService = bindings.services.time as MockTimeService;
    timeService.advance(61);

    // Heartbeat once, this should subtract the credit rate.
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId)
      .submit();
    const updatedAccount = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    expect(updatedAccount.credits).toBe(account.credits - creditRate);

    // Heartbeat again, this shouldn't do anything.
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId)
      .submit();
    const latestAccount = await clients.admin
      .getAccount(workloads[0].accountId)
      .submit();
    expect(latestAccount.credits).toBe(updatedAccount.credits);

    otherAccount = await clients.admin
      .getAccount(otherAccount.accountId)
      .submit();
    // should be the original minus 1 credit taken by the one workload we're running
    expect(otherAccount.credits).toBe(1500 - 1);
  });
});
