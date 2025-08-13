import { z } from "zod";

export const Account = z
  .object({
    id: z.string(),
    name: z.string().max(32),
    apiToken: z.string(),
    createdAt: z.string().datetime(),
  })
  .openapi({
    ref: "Account",
  });
export type Account = z.infer<typeof Account>;

export const CreateAccountRequest = z
  .object({
    name: z.string(),
  })
  .openapi({
    ref: "CreateAccountRequest",
  });
export type CreateAccountRequest = z.infer<typeof CreateAccountRequest>;
