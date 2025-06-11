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
    hostname: "my-metal-instance",
    totalMemory: 128,
    osReservedMemory: 8,
    totalCpus: 64,
    osReservedCpus: 4,
    totalDisk: 1024,
    osReservedDisk: 100,
  };

  const createWorkloadRequest: CreateWorkloadRequest = {
    name: "my-cool-workload",
    description: "This is a test workload",
    tags: ["test", "workload"],
    dockerCompose:
      "version: '3'\nservices:\n  app:\n    image: nginx\n    ports:\n      - '80:80'",
    envVars: {
      MY_SECRET: "42",
    },
    serviceToExpose: "app",
    servicePortToExpose: 80,
    memory: 4,
    cpus: 2,
    disk: 40,
  };

  it("should register a metal instance that haven't been created", async ({
    expect,
    metalInstanceClient,
  }) => {
    const getResponse = await metalInstanceClient.get({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(404);

    const response = await metalInstanceClient.register(myMetalInstance);
    expect(response.status).equals(200);
    const getResponseAfter = await metalInstanceClient.get({
      id: myMetalInstance.id,
    });

    expect(getResponseAfter.response.status).equals(200);
    const body = await getResponseAfter.parse_body();
    const cleanBody = { ...body, updatedAt: undefined, createdAt: undefined };
    expect(cleanBody).toEqual(myMetalInstance);
  });

  it("should register a metal instance that already exists, updating it", async ({
    expect,
    metalInstanceClient,
  }) => {
    const updatedMetalInstance = {
      ...myMetalInstance,
      totalMemory: 256,
      totalCpus: 128,
    };
    const response = await metalInstanceClient.register(updatedMetalInstance);
    expect(response.status).equals(200);
    const getResponse = await metalInstanceClient.get({
      id: myMetalInstance.id,
    });
    expect(getResponse.response.status).equals(200);
    const body = await getResponse.parse_body();
    const cleanBody = { ...body, updatedAt: undefined, createdAt: undefined };
    expect(cleanBody).toEqual(updatedMetalInstance);
  });

  it("should sync the metal instance", async ({
    expect,
    metalInstanceClient,
    workloadClient,
  }) => {
    const createWokloadResponse = await workloadClient.create(
      createWorkloadRequest,
    );
    const createWokload = await createWokloadResponse.parse_body();
    const syncResponse = await metalInstanceClient.sync({
      id: myMetalInstance.id,
      workloads: [],
    });
    expect(syncResponse.response.status).equals(200);
    const body = await syncResponse.parse_body();
    expect(body.workloads.length).toEqual(1);

    const sync2Response = await metalInstanceClient.sync({
      id: myMetalInstance.id,
      workloads: [
        {
          id: createWokload.id,
          status: "running",
        },
      ],
    });
    expect(sync2Response.response.status).equals(200);
    const workloadAfterSync = await workloadClient.get({
      id: createWokload.id,
    });
    expect(workloadAfterSync.response.status).equals(200);
    const workloadAfterSyncBody = await workloadAfterSync.parse_body();
    expect(workloadAfterSyncBody.status).toEqual("running");
  });
});
