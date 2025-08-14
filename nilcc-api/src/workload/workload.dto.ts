import { z } from "zod";
import { SystemLogsRequest } from "#/clients/nilcc-agent.client";
import { Uuid } from "#/common/types";

const FILENAME_REGEX = /^[\w/._-]+$/;

export const CreateWorkloadRequest = z
  .object({
    name: z
      .string()
      .min(1, "name cannot be empty")
      .openapi({
        description: "A descriptive name for the workload",
        examples: ["my-favorite-workload"],
      }),
    dockerCompose: z.string().openapi({
      description:
        "The docker compose to be ran. The docker compose can contain any number of services but it must contain a single one that will act as the public entry point to the CVM.",
      examples: [
        `services:
  api:
    image: caddy:2
    command: |
      caddy respond --listen :80 --body '{"hi":"foo"}' --header "Content-Type: application/json"`,
      ],
    }),
    envVars: z
      .record(z.string(), z.string())
      .optional()
      .openapi({
        description:
          "The optional environment variables to set on this workload. Environment variables are private and are not included in the attestation measurement.",
        examples: [{ FOO: "42" }],
      }),
    files: z
      .record(z.string(), z.string().base64())
      .refine(
        (arg) => Object.keys(arg).every((name) => name.match(FILENAME_REGEX)),
        `filename must follow $the pattern ${FILENAME_REGEX}`,
      )
      .optional()
      .openapi({
        description:
          "The optional set of files that are meant to be mounted in the docker compose file. These are available under a special `$FILES` prefix that must be used in the docker compose file when referencing these files as mounts. Note that the file contents must be encoded in base64.",
        examples: [
          {
            "foo/bar.txt":
              "dGhpcyBpcyBhIGZpbGUgY3JlYXRlZCBpbnNpZGUgdGhlIENWTSBhbmQgbW91bnRlZCB2aWEgZG9ja2VyIGNvbXBvc2U=",
          },
        ],
      }),
    publicContainerName: z
      .string()
      .min(1, "public container name cannot be empty")
      .openapi({
        description:
          "The container that acts as an entry point to this workload, which must be a part of the docker compose definition.",
        examples: ["api"],
      }),
    publicContainerPort: z
      .number()
      .int()
      .positive()
      .openapi({
        description:
          "The port that the public container uses to expose its service. This must contain the port this container is bound to, whether it is exposed or not.",
        examples: [80],
      }),
    memory: z
      .number()
      .int()
      .positive()
      .openapi({
        description:
          "The amount of memory, in MBs, that the CVM should allocate for this workload.",
        examples: [2048],
      }),
    cpus: z.number().int().positive().openapi({
      description:
        "The number of CPUs that the CVM should allocate for this workload.",
    }),
    disk: z
      .number()
      .int()
      .min(5, "Disk must be at least 5GB")
      .max(100, "Disk must be at most 100GB")
      .openapi({
        description:
          "The disk space, in GBs, that the CVM should allocate for this workload. This disk space is used towards anything that's stored in the filesystem during runtime, including docker images, docker containers, files that containers will write, etc. When using large docker images, this parameter should be high enough to accommodate for them.",
        examples: [10],
      }),
    gpus: z.number().int().openapi({
      description:
        "The number of GPUs to that the CVM should allocate for this workload.",
    }),
  })
  .openapi({
    ref: "CreateWorkloadRequest",
    description: "A request to create a workload",
  });
export type CreateWorkloadRequest = z.infer<typeof CreateWorkloadRequest>;

export const CreateWorkloadResponse = CreateWorkloadRequest.extend({
  id: Uuid.openapi({ description: "The identifier for this workload." }),
  status: z
    .enum(["scheduled", "starting", "running", "stopped", "error"])
    .openapi({ description: "The status of the workload." }),
  createdAt: z.string().datetime().openapi({
    description: "The timestamp at which this workload was created.",
  }),
  updatedAt: z.string().datetime().openapi({
    description: "The timestamp at which this workload was last created.",
  }),
  domain: z.string().openapi({
    description: "The domain where this workload is reachable via https.",
  }),
  account: z.string().openapi({
    description: "The account this workload belongs to.",
  }),
}).openapi({ ref: "CreateWorkloadResponse" });
export type CreateWorkloadResponse = z.infer<typeof CreateWorkloadResponse>;

export const DeleteWorkloadRequest = z
  .object({
    id: Uuid.openapi({
      description: "The identifier for the workload to be deleted.",
    }),
  })
  .openapi({ ref: "DeleteWorkloadRequest" });
export type DeleteWorkloadRequest = z.infer<typeof DeleteWorkloadRequest>;

export const GetWorkloadResponse = CreateWorkloadResponse.openapi({
  ref: "GetWorkloadResponse",
});
export type GetWorkloadResponse = z.infer<typeof GetWorkloadResponse>;

export const ListWorkloadsResponse = z
  .array(GetWorkloadResponse)
  .openapi({ ref: "ListWorkloadsResponse" });
export type ListWorkloadsResponse = z.infer<typeof ListWorkloadsResponse>;

export const WorkloadSystemLogsRequest = SystemLogsRequest.extend({
  id: Uuid.openapi({
    description: "The identifier for the workloads to get system logs from.",
  }),
}).openapi({ ref: "WorkloadSystemLogsRequest" });
export type WorkloadSystemLogsRequest = z.infer<
  typeof WorkloadSystemLogsRequest
>;

export const WorkloadSystemLogsResponse = z
  .object({
    lines: z.string().array().openapi({
      description: "The system log lines.",
    }),
  })
  .openapi({ ref: "WorkloadSystemLogsResponse" });
export type WorkloadSystemLogsResponse = z.infer<
  typeof WorkloadSystemLogsResponse
>;
