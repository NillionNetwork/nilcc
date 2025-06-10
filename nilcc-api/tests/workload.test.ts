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
    dockerCompose:
      "version: '3'\nservices:\n  app:\n    image: nginx\n    ports:\n      - '80:80'",
    envVars: {
      MY_SECRET: "42",
    },
    serviceToExpose: "app",
    servicePortToExpose: 80,
    memory: 4,
    cpu: 2,
    disk: 40,
  };

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

  it("should fail to create a workload if there isn't a metal instance", async ({
    expect,
    workloadClient,
  }) => {
    const myWorkloadResponse = await workloadClient.create(
      createWorkloadRequest,
    );
    expect(myWorkloadResponse.response.status).equal(503);
  });

  it("should create a workload", async ({
    expect,
    workloadClient,
    metalInstanceClient,
  }) => {
    await metalInstanceClient.register(myMetalInstance);

    const myWorkloadResponse = await workloadClient.create(
      createWorkloadRequest,
    );
    myWorkload = await myWorkloadResponse.parse_body();
    expect(myWorkload.name).equals(createWorkloadRequest.name);
  });

  it("should fail to create a workload if it doesn't fit in the metal instance", async ({
    expect,
    workloadClient,
  }) => {
    const overloadedWorkloadRequest = {
      ...createWorkloadRequest,
      cpu: 63, // Exceeding the available CPU
    };
    const myWorkloadResponse = await workloadClient.create(
      overloadedWorkloadRequest,
    );
    expect(myWorkloadResponse.response.status).equal(503);
  });

  it("should get a workload", async ({ expect, workloadClient }) => {
    const myWorkloadResponse = await workloadClient.get({
      id: myWorkload!.id,
    });
    const workloadData = await myWorkloadResponse.parse_body();
    expect(workloadData.name).equals(myWorkload!.name);
  });

  it("should list the workloads", async ({ expect, workloadClient }) => {
    const workloadsResponse = await workloadClient.list();
    const workloads = await workloadsResponse.parse_body();
    expect(workloads.length).greaterThan(0);
    expect(workloads[0].name).equals(myWorkload!.name);
  });

  it("should update a workload", async ({ expect, workloadClient }) => {
    const updatedName = "my-cool-workload-updated";
    const response = await workloadClient.update({
      id: myWorkload!.id,
      name: updatedName,
    });
    expect(response.status).equals(200);
  });

  it("should delete a workload", async ({ expect, workloadClient }) => {
    const response = await workloadClient.delete({
      id: myWorkload!.id,
    });
    expect(response.status).equals(200);

    // Verify deletion
    const getResponse = await workloadClient.get({
      id: myWorkload!.id,
    });
    expect(getResponse.response.status).equal(404);
  });
});
