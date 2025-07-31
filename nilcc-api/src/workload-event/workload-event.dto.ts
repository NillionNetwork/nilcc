import { z } from "zod";
import { Uuid } from "#/common/types";
import { WorkloadEventKind } from "#/metal-instance/metal-instance.dto";

export const WorkloadEvent = z
  .object({
    id: Uuid,
    details: WorkloadEventKind,
    timestamp: z.string().datetime(),
  })
  .openapi({ ref: "WorkloadEvent" });
export type WorkloadEvent = z.infer<typeof WorkloadEvent>;

export const ListWorkloadEventsRequest = z
  .object({
    workloadId: Uuid,
  })
  .openapi({ ref: "ListWorkloadEventsRequest" });
export type ListWorkloadEventsRequest = z.infer<
  typeof ListWorkloadEventsRequest
>;

export const ListWorkloadEventsResponse = z
  .object({
    events: WorkloadEvent.array(),
  })
  .openapi({ ref: "ListWorkloadEventsResponse" });
export type ListWorkloadEventsResponse = z.infer<
  typeof ListWorkloadEventsResponse
>;
