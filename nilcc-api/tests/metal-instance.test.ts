import { describe } from "vitest";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
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
    metalInstanceClient,
    userClient,
  }) => {
    const getResponse = await userClient.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(404);

    const response = await metalInstanceClient.register(myMetalInstance);
    expect(response.status).equals(200);
    const getResponseAfter = await userClient.getMetalInstance({
      id: myMetalInstance.id,
    });

    expect(getResponseAfter.response.status).equals(200);
    const body = await getResponseAfter.parse_body();
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
    metalInstanceClient,
    userClient,
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
    const response = await metalInstanceClient.register(updatedMetalInstance);
    expect(response.status).equals(200);
    const getResponse = await userClient.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(200);
    const body = await getResponse.parse_body();
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
    metalInstanceClient,
    userClient,
  }) => {
    await metalInstanceClient.register(myMetalInstance);
    const originalResponse = await userClient.getMetalInstance({
      id: myMetalInstance.id,
    });
    expect(originalResponse.response.status).equals(200);
    const originalBody = await originalResponse.parse_body();
    const lastSeen = new Date(originalBody.lastSeenAt);
    // sleep for a little bit
    await new Promise((resolve) => setTimeout(resolve, 200));

    const heartbeatResponse = await metalInstanceClient.heartbeat({
      id: myMetalInstance.id,
    });
    expect(heartbeatResponse.status).equals(200);

    // now the last seen timestamp should be larger
    const response = await userClient.getMetalInstance({
      id: myMetalInstance.id,
    });
    const body = await response.parse_body();
    const currentLastSeen = new Date(body.lastSeenAt);
    expect(currentLastSeen.getTime()).toBeGreaterThan(lastSeen.getTime());
  });
});
