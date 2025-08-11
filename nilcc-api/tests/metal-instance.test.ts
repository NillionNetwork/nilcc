import { describe } from "vitest";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
import type { CreateWorkloadRequest } from "#/workload/workload.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("Metal Instance", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  const myMetalInstance: RegisterMetalInstanceRequest = {
    id: "c92c86e4-c7e5-4bb3-a5f5-45945b5593e4",
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
    const getResponse = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(404);

    const response = await clients.metalInstance.register(myMetalInstance);
    expect(response.status).equals(200);
    const getResponseAfter = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });

    expect(getResponseAfter.response.status).equals(200);
    const body = await getResponseAfter.parseBody();
    const cleanBody = {
      ...body,
      updatedAt: undefined,
      createdAt: undefined,
      lastSeenAt: undefined,
      token: myMetalInstance.token,
    };
    expect(cleanBody).toEqual(myMetalInstance);
  });

  it("should register a metal instance that already exists, updating it", async ({
    expect,
    clients,
  }) => {
    const updatedMetalInstance = {
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
    const response = await clients.metalInstance.register(updatedMetalInstance);
    expect(response.status).equals(200);
    const getResponse = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(200);
    const body = await getResponse.parseBody();
    const cleanBody = {
      ...body,
      updatedAt: undefined,
      createdAt: undefined,
      lastSeenAt: undefined,
      token: myMetalInstance.token,
    };
    expect(cleanBody).toEqual(updatedMetalInstance);
  });

  it("should update the last seen timestamp after a heartbeat", async ({
    expect,
    clients,
  }) => {
    await clients.metalInstance.register(myMetalInstance);
    const originalResponse = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(originalResponse.response.status).equals(200);
    const originalBody = await originalResponse.parseBody();
    const lastSeen = new Date(originalBody.lastSeenAt);
    // sleep for a little bit
    await new Promise((resolve) => setTimeout(resolve, 200));

    const heartbeatResponse = await clients.metalInstance.heartbeat({
      id: myMetalInstance.id,
    });
    expect(heartbeatResponse.status).equals(200);

    // now the last seen timestamp should be larger
    const response = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });
    const body = await response.parseBody();
    const currentLastSeen = new Date(body.lastSeenAt);
    expect(currentLastSeen.getTime()).toBeGreaterThan(lastSeen.getTime());
  });

  it("should allow deleting metal instances", async ({ expect, clients }) => {
    const originalResponse = await clients.admin.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(originalResponse.response.status).equals(200);
    const instance = await originalResponse.parseBody();

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
    const workloadResponse = await clients.user.createWorkload(
      createWorkloadRequest,
    );
    expect(workloadResponse.response.status).equals(200);
    const workload = await workloadResponse.parseBody();

    const firstDeleteResponse = await clients.admin.deleteMetalInstance(
      instance.id,
    );
    expect(firstDeleteResponse.status).toBe(412);

    const deleteWorkloadResponse = await clients.user.deleteWorkload({
      id: workload.id,
    });
    expect(deleteWorkloadResponse.status).toBe(200);

    const response = await clients.admin.deleteMetalInstance(instance.id);
    expect(response.status).toBe(200);
  });
});
