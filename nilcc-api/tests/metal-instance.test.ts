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
    endpoint: "http://127.0.0.1:35433",
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
      token: myMetalInstance.token,
    };
    expect(cleanBody).toEqual(updatedMetalInstance);
  });
});
