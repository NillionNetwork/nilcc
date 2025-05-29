import z from "zod";
import type { App } from "#/app";
import type { AppBindings } from "#/env";

export type ControllerOptions = {
  app: App;
  bindings: AppBindings;
};

export const Uuid = z.string().uuid().openapi({
  description: "UUID v4",
  type: "string",
  format: "uuid",
});
