import { describe } from "vitest";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
import type { CreateWorkloadRequest } from "#/workload/workload.dto";
import type { MockTimeService } from "./fixture/fixture";
import { createTestFixtureExtension } from "./fixture/it";

describe("Metal Instance", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

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
      total: 128,
      reserved: 16,
    },
    gpus: 0,
  };

  it("should register a metal instance that hasn't been created", async ({
    expect,
    clients,
  }) => {
    expect(
      await clients.admin
        .getMetalInstance(myMetalInstance.metalInstanceId)
        .status(),
    ).equals(404);

    await clients.metalInstance.register(myMetalInstance).submit();

    const instance = await clients.admin
      .getMetalInstance(myMetalInstance.metalInstanceId)
      .submit();
    const cleanInstance = {
      ...instance,
      updatedAt: undefined,
      createdAt: undefined,
      lastSeenAt: undefined,
      token: myMetalInstance.token,
    };
    expect(cleanInstance).toEqual(myMetalInstance);
  });

  it("should register a metal instance that already exists, updating it", async ({
    expect,
    clients,
  }) => {
    const updatedInstance = {
      ...myMetalInstance,
      memoryMb: {
        total: 16384,
        reserved: 1024,
      },
      cpus: {
        total: 80,
        reserved: 20,
      },
    };
    await clients.metalInstance.register(updatedInstance).submit();

    const instance = await clients.admin
      .getMetalInstance(myMetalInstance.metalInstanceId)
      .submit();
    const cleanInstance = {
      ...instance,
      updatedAt: undefined,
      createdAt: undefined,
      lastSeenAt: undefined,
      token: myMetalInstance.token,
    };
    expect(cleanInstance).toEqual(updatedInstance);
  });

  it("should update the last seen timestamp after a heartbeat", async ({
    bindings,
    expect,
    clients,
  }) => {
    const instance = await clients.admin
      .getMetalInstance(myMetalInstance.metalInstanceId)
      .submit();
    const lastSeen = new Date(instance.lastSeenAt);

    // Move the clock forward by a bit
    const timeService = bindings.services.time as MockTimeService;
    timeService.advance(1);
    await clients.metalInstance
      .heartbeat(myMetalInstance.metalInstanceId)
      .submit();

    // now the last seen timestamp should have moved the same amount
    const updatedInstance = await clients.admin
      .getMetalInstance(myMetalInstance.metalInstanceId)
      .submit();
    const currentLastSeen = new Date(updatedInstance.lastSeenAt);
    expect(currentLastSeen.getTime()).toBe(lastSeen.getTime() + 1000);
  });

  it("should allow deleting metal instances", async ({ expect, clients }) => {
    const instance = await clients.admin
      .getMetalInstance(myMetalInstance.metalInstanceId)
      .submit();

    const createWorkloadRequest: CreateWorkloadRequest = {
      name: "my-cool-workload",
      dockerCompose: `
services:
  app:
    image: nginx
    ports:
      - '80'
`,
      publicContainerName: "app",
      publicContainerPort: 80,
      memory: 4,
      cpus: 2,
      disk: 40,
      gpus: 0,
    };
    await clients.admin
      .createTier({
        name: "tiny",
        cost: 1,
        cpus: 2,
        gpus: 0,
        memoryMb: 4,
        diskGb: 40,
      })
      .submit();
    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();

    // we shouldn't be able to delete it since it's running a workload
    expect(
      await clients.admin
        .deleteMetalInstance(instance.metalInstanceId)
        .status(),
    ).toBe(412);

    // now delete the workload and successfully delete the instance
    await clients.user.deleteWorkload(workload.workloadId).submit();
    clients.admin.deleteMetalInstance(instance.metalInstanceId).submit();
  });
});
