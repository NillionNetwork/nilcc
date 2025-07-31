import z from "zod";
import { Container, ContainerLogsRequest } from "#/clients/nilcc-agent.client";
import { Uuid } from "#/common/types";

export const ListContainersRequest = z
  .object({ workloadId: Uuid })
  .openapi({ ref: "ListContainersRequest" });
export type ListContainersRequest = z.infer<typeof ListContainersRequest>;

export const ListContainersResponse = z
  .array(Container)
  .openapi({ ref: "ListContainersResponse" });
export type ListContainersResponse = z.infer<typeof ListContainersResponse>;

export const WorkloadContainerLogsRequest = ContainerLogsRequest.extend({
  workloadId: z.string(),
}).openapi({ ref: "WorkloadContainerLogsRequest" });
export type WorkloadContainerLogsRequest = z.infer<
  typeof WorkloadContainerLogsRequest
>;

export const WorkloadContainerLogsResponse = z
  .object({ lines: z.string().array() })
  .openapi({ ref: "WorkloadContainerLogsResponse" });
export type WorkloadContainerLogsResponse = z.infer<
  typeof WorkloadContainerLogsResponse
>;
