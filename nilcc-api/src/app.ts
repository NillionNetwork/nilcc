import "./effects";

import { prometheus } from "@hono/prometheus";
import { Hono } from "hono";
import { bodyLimit } from "hono/body-limit";
import { timeout } from "hono/timeout";
import { Temporal } from "temporal-polyfill";
import { errorHandler } from "#/common/handler";
import { buildAccountRouter } from "./account/account.router";
import {
  type AppBindings,
  type AppEnv,
  FeatureFlag,
  hasFeatureFlag,
} from "./env";
import { buildMetalInstanceRouter } from "./metal-instance/metal-instance.router";
import { createOpenApiRouter } from "./openapi/openapi.router";
import { buildSystemRouter } from "./system/system.router";
import { buildWorkloadRouter } from "./workload/workload.router";
import { buildWorkloadContainerRouter } from "./workload-container/workload-container.router";
import { buildWorkloadEventRouter } from "./workload-event/workload-event.router";
import { buildWorkloadTierRouter } from "./workload-tier/workload-tier.router";

export type App = Hono<AppEnv>;

export async function buildApp(
  bindings: AppBindings,
): Promise<{ app: App; metrics: Hono }> {
  const app = new Hono<AppEnv>();
  const metricsApp = new Hono();

  const { printMetrics, registerMetrics } = prometheus();
  app.use("*", bodyLimit({ maxSize: 16 * 1024 * 1024 }));
  app.use("*", registerMetrics);

  app.use((c, next) => {
    c.env = bindings;
    return next();
  });

  if (
    hasFeatureFlag(bindings.config.enabledFeatures, FeatureFlag.OPENAPI_SPEC)
  ) {
    createOpenApiRouter({ app, bindings });
  }

  buildAccountRouter({ app, bindings });
  buildWorkloadRouter({ app, bindings });
  buildWorkloadContainerRouter({ app, bindings });
  buildWorkloadEventRouter({ app, bindings });
  buildWorkloadTierRouter({ app, bindings });
  buildMetalInstanceRouter({ app, bindings });
  buildSystemRouter({ app, bindings });

  metricsApp.get("/metrics", printMetrics);

  app.get("/health", (c) => {
    return c.json({ status: "ok", time: new Date().toISOString() });
  });

  const limit = Temporal.Duration.from({ minutes: 1 }).total("milliseconds");
  app.use("*", timeout(limit));

  app.onError(errorHandler);

  return { app, metrics: metricsApp };
}
