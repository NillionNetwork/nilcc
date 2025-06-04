import { describe } from "vitest";
import type { CreateWorkloadResponse } from "#/workload/workload.dto";
import { createTestFixtureExtension } from "./fixture/it";

describe("workload CRUD", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});
  let myWorkload: null | CreateWorkloadResponse = null;

  it("should create a workload", async ({ expect, workloadClient }) => {
    const name = "my-cool-workload";
    const myWorkloadResponse = await workloadClient.create({
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
