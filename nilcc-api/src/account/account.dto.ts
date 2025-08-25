import { z } from "zod";

export const Account = z
  .object({
    accountId: z.string().openapi({ description: "The account identifier." }),
    name: z.string().max(32).openapi({ description: "The account name." }),
    apiToken: z
      .string()
      .openapi({ description: "The token to use when talking to the API." }),
    createdAt: z
      .string()
      .datetime()
      .openapi({ description: "The timestamp when this account was created." }),
    credits: z
      .number()
      .openapi({ description: "The number of credits this account has." }),
  })
  .openapi({
    ref: "Account",
  });
export type Account = z.infer<typeof Account>;

export const CreateAccountRequest = z
  .object({
    name: z.string(),
    credits: z.number().default(0),
  })
  .openapi({
    ref: "CreateAccountRequest",
  });
export type CreateAccountRequest = z.infer<typeof CreateAccountRequest>;

export const AddCreditsRequest = z
  .object({
    accountId: z.string(),
    credits: z.number(),
  })
  .openapi({
    ref: "AddCreditsRequest",
  });
export type AddCreditsRequest = z.infer<typeof AddCreditsRequest>;
