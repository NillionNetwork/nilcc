import { z } from "zod";
import { Uuid } from "#/common/types";

export const WorkloadEventKind = z.discriminatedUnion("kind", [
  z.object({ kind: z.literal("created") }),
  z.object({ kind: z.literal("starting") }),
  z.object({ kind: z.literal("stopped") }),
  z.object({ kind: z.literal("vmRestarted") }),
  z.object({ kind: z.literal("forcedRestart") }),
  z.object({ kind: z.literal("awaitingCert") }),
  z.object({ kind: z.literal("running") }),
  z.object({ kind: z.literal("failedToStart"), error: z.string() }),
  z.object({ kind: z.literal("warning"), message: z.string() }),
]);
export type WorkloadEventKind = z.infer<typeof WorkloadEventKind>;

export const WorkloadEvent = z
  .object({
    eventId: Uuid,
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

export const SubmitEventRequest = z
  .object({
    metalInstanceId: Uuid,
    workloadId: Uuid,
    event: WorkloadEventKind,
    timestamp: z.string().datetime(),
  })
  .openapi({ ref: "SubmitEventRequest" });
export type SubmitEventRequest = z.infer<typeof SubmitEventRequest>;
