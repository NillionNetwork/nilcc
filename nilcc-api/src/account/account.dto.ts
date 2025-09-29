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

export const MyAccount = Account.omit({ apiToken: true })
  .extend({
    creditRate: z.number().openapi({
      description: "The amount of credits currently being burnt per minute.",
    }),
  })
  .openapi({
    ref: "MyAccount",
  });
export type MyAccount = z.infer<typeof MyAccount>;

export const CreateAccountRequest = z
  .object({
    name: z.string(),
    credits: z.number().default(0),
  })
  .openapi({
    ref: "CreateAccountRequest",
  });
export type CreateAccountRequest = z.infer<typeof CreateAccountRequest>;

export const UpdateAccountRequest = z
  .object({
    accountId: z.string().openapi({ description: "The account identifier." }),
    name: z.string().openapi({ description: "The new name for this account." }),
  })
  .openapi({
    ref: "UpdateAccountRequest",
  });
export type UpdateAccountRequest = z.infer<typeof UpdateAccountRequest>;

export const AddCreditsRequest = z
  .object({
    accountId: z.string(),
    credits: z.number(),
  })
  .openapi({
    ref: "AddCreditsRequest",
  });
export type AddCreditsRequest = z.infer<typeof AddCreditsRequest>;

export const AccountCreditsResponse = z
  .object({
    accountId: z.string(),
    credits: z.number(),
  })
  .openapi({
    ref: "AccountCreditsResponse",
  });
export type AccountCreditsResponse = z.infer<typeof AccountCreditsResponse>;
