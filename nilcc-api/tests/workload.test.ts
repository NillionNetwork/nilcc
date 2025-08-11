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
    publicContainerName: "app",
    publicContainerPort: 80,
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
    clients,
  }) => {
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
    expect(workload.domain).equals(`${workload.id}.workloads.public.localhost`);
    // store it for other tests to re-use it
    myWorkload = workload;
  });

  it("should fail to create a workload if it doesn't fit in the metal instance", async ({
    expect,
    clients,
  }) => {
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
    const workload = await clients.user.getWorkload(myWorkload!.id).submit();
    expect(workload.name).equals(myWorkload!.name);
  });

  it("should list the workloads", async ({ expect, clients }) => {
    const workloads = await clients.user.listWorkloads().submit();
    expect(workloads.length).greaterThan(0);
    expect(workloads[0].name).equals(myWorkload!.name);
  });

  it("should delete a workload", async ({ expect, clients }) => {
    await clients.user.deleteWorkload(myWorkload!.id).submit();

    // Verify deletion
    const status = await clients.user.getWorkload(myWorkload!.id).status();
    expect(status).equal(404);
  });

  it("should update a workload's state", async ({ expect, clients }) => {
    const workload = await clients.user
      .createWorkload(createWorkloadRequest)
      .submit();

    await clients.metalInstance
      .submitEvent({
        agentId: myMetalInstance.id,
        workloadId: workload.id,
        event: { kind: "starting" },
      })
      .submit();

    const updatedWorkload = await clients.user
      .getWorkload(workload!.id)
      .submit();
    expect(updatedWorkload.status).toBe("starting");

    const eventsBody = await clients.user
      .getWorkloadEvents(workload!.id)
      .submit();
    expect(eventsBody.events).toHaveLength(2);
    const eventKinds = eventsBody.events.map((e) => e.details.kind);
    expect(eventKinds).toEqual(["created", "starting"]);
  });
});
