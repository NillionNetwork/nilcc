import { describe } from "vitest";
import type { CreateWorkloadResponse } from "#/workload/workload.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("workload CRUD", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});
  let myWorkload: null | CreateWorkloadResponse = null;

  it("should create a workload", async ({ expect, workload }) => {
    const name = "my-cool-workload";
    const myWorkloadResponse = await workload.createWorkload({
      name,
      description: "This is a test workload",
      tags: ["test", "workload"],
      dockerCompose:
        "version: '3'\nservices:\n  app:\n    image: nginx\n    ports:\n      - '80:80'",
      serviceToExpose: "app",
      servicePortToExpose: 80,
      memory: 4,
      cpu: 2,
    });
    myWorkload = await myWorkloadResponse.parse_body();
    expect(myWorkload.name).equals(name);
  });

  it("should get a workload", async ({ expect, workload }) => {
    const myWorkloadResponse = await workload.getWorkload({
      id: myWorkload!.id,
    });
    const workloadData = await myWorkloadResponse.parse_body();
    expect(workloadData.name).equals(myWorkload!.name);
  });

  it("should list the workloads", async ({ expect, workload }) => {
    const workloadsResponse = await workload.listWorkloads();
    const workloads = await workloadsResponse.parse_body();
    expect(workloads.length).greaterThan(0);
    expect(workloads[0].name).equals(myWorkload!.name);
  });

  it("should update a workload", async ({ expect, workload }) => {
    const updatedName = "my-cool-workload-updated";
    const response = await workload.updateWorkload({
      id: myWorkload!.id,
      name: updatedName,
    });
    const status = response.status;
    expect(status).equals(200);
  });

  it("should delete a workload", async ({ expect, workload }) => {
    const response = await workload.deleteWorkload({
      id: myWorkload!.id,
    });
    const status = response.status;
    expect(status).equals(200);

    // Verify deletion
    const getResponse = await workload.getWorkload({
      id: myWorkload!.id,
    });
    expect(getResponse.response.status).equal(404);
  });
});
