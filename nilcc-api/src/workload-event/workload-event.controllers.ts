import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { metalInstanceAuthentication, userAuthentication } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator, responseValidator } from "#/common/zod-utils";
import { transactionMiddleware } from "#/data-source";
import {
  ListWorkloadEventsRequest,
  ListWorkloadEventsResponse,
  SubmitEventRequest,
} from "./workload-event.dto";

export function submitEvent(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadEvents.submit,
    describeRoute({
      tags: ["metal-instance", "workload"],
      summary: "Report an event for a workload",
      description:
        "This endpoint is used by nilcc-agent to asynchronously report events for a workload. This includes events like errors and a workload starting, stopping, etc.",
      responses: {
        200: {
          description: "The event was processed successfully",
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    metalInstanceAuthentication(bindings),
    payloadValidator(SubmitEventRequest),
    transactionMiddleware(bindings.dataSource),
    async (c) => {
      const payload = c.req.valid("json");
      await bindings.services.workload.submitEvent(
        bindings,
        payload,
        c.get("txQueryRunner"),
      );
      return c.json({});
    },
  );
}

export function listEvents(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.workloadEvents.list,
    describeRoute({
      tags: ["metal-instance", "workload"],
      summary: "List the events for a workload",
      responses: {
        200: {
          description: "The list of events for this workload",
          content: {
            "application/json": {
              schema: resolver(ListWorkloadEventsResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    payloadValidator(ListWorkloadEventsRequest),
    transactionMiddleware(bindings.dataSource),
    responseValidator(bindings, ListWorkloadEventsResponse),
    async (c) => {
      const payload = c.req.valid("json");
      const events = await bindings.services.workload.listEvents(
        bindings,
        payload,
        c.get("account"),
        c.get("txQueryRunner"),
      );
      return c.json({ events });
    },
  );
}
