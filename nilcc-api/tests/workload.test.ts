import { describe } from "vitest";
import type { RegisterMetalInstanceRequest } from "#/metal-instance/metal-instance.dto";
import type {
  CreateWorkloadRequest,
  CreateWorkloadResponse,
} from "#/workload/workload.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("workload CRUD", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});
  let myWorkload: null | CreateWorkloadResponse = null;

  const createWorkloadRequest: CreateWorkloadRequest = {
    name: "my-cool-workload",
    description: "This is a test workload",
    tags: ["test", "workload"],
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
    serviceToExpose: "app",
    servicePortToExpose: 80,
    memory: 4,
    cpus: 2,
    disk: 40,
    gpus: 0,
  };

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
      total: 1024,
      reserved: 128,
    },
    gpus: 0,
  };

  it("should fail to create a workload if there isn't a metal instance", async ({
    expect,
    userClient,
  }) => {
    const myWorkloadResponse = await userClient.createWorkload(
      createWorkloadRequest,
    );
    expect(myWorkloadResponse.response.status).equal(503);
  });

  it("should create a workload", async ({
    expect,
    userClient,
    metalInstanceClient,
  }) => {
    await metalInstanceClient.register(myMetalInstance);

    const myWorkloadResponse = await userClient.createWorkload(
      createWorkloadRequest,
    );
    myWorkload = await myWorkloadResponse.parse_body();
    expect(myWorkload.name).equals(createWorkloadRequest.name);
  });

  it("should fail to create a workload if it doesn't fit in the metal instance", async ({
    expect,
    userClient,
  }) => {
    const overloadedWorkloadRequest = {
      ...createWorkloadRequest,
      cpus: 63, // Exceeding the available CPU
    };
    const myWorkloadResponse = await userClient.createWorkload(
      overloadedWorkloadRequest,
    );
    expect(myWorkloadResponse.response.status).equal(503);
  });

  it("should get a workload", async ({ expect, userClient }) => {
    const myWorkloadResponse = await userClient.getWorkload({
      id: myWorkload!.id,
    });
    const workloadData = await myWorkloadResponse.parse_body();
    expect(workloadData.name).equals(myWorkload!.name);
  });

  it("should list the workloads", async ({ expect, userClient }) => {
    const workloadsResponse = await userClient.listWorkloads();
    const workloads = await workloadsResponse.parse_body();
    expect(workloads.length).greaterThan(0);
    expect(workloads[0].name).equals(myWorkload!.name);
  });

  it("should delete a workload", async ({ expect, userClient }) => {
    const response = await userClient.deleteWorkload({
      id: myWorkload!.id,
    });
    expect(response.status).equals(200);

    // Verify deletion
    const getResponse = await userClient.getWorkload({
      id: myWorkload!.id,
    });
    expect(getResponse.response.status).equal(404);
  });

  it("should update a workload's state", async ({
    expect,
    userClient,
    metalInstanceClient,
  }) => {
    await metalInstanceClient.register(myMetalInstance);
    const workloadResponse = await userClient.createWorkload(
      createWorkloadRequest,
    );
    myWorkload = await workloadResponse.parse_body();

    const response = await userClient.submitEvent({
      agentId: myMetalInstance.id,
      workloadId: myWorkload.id,
      event: { kind: "starting" },
    });
    expect(response.status).toBe(200);

    const updatedWorkloadResponse = await userClient.getWorkload({
      id: myWorkload.id,
    });
    const updatedWorkload = await updatedWorkloadResponse.parse_body();
    expect(updatedWorkload.status).toBe("starting");
  });
});
