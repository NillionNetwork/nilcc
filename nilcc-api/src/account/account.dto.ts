import { z } from "zod";

export const Account = z
  .object({
    accountId: z.string().openapi({ description: "The account identifier." }),
    name: z.string().max(32).openapi({ description: "The account name." }),
    walletAddress: z.string().openapi({
      description: "The Ethereum wallet address for this account.",
    }),
    createdAt: z.string().datetime().openapi({
      description: "The timestamp when this account was created.",
    }),
    balance: z.number().openapi({
      description: "The NIL balance (decimal, e.g. 5.5 means 5.5 NIL).",
    }),
  })
  .openapi({
    ref: "Account",
  });
export type Account = z.infer<typeof Account>;

export const MyAccount = Account.extend({
  burnRatePerMin: z.number().openapi({
    description:
      "The amount of NIL currently being burnt per minute across all running workloads.",
  }),
}).openapi({
  ref: "MyAccount",
});
export type MyAccount = z.infer<typeof MyAccount>;

export const CreateAccountRequest = z
  .object({
    name: z.string(),
    walletAddress: z.string().regex(/^0x[a-fA-F0-9]{40}$/),
    balance: z.number().default(0),
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

export const AddBalanceRequest = z
  .object({
    accountId: z.string(),
    balance: z.number(),
  })
  .openapi({
    ref: "AddBalanceRequest",
  });
export type AddBalanceRequest = z.infer<typeof AddBalanceRequest>;

export const AccountBalanceResponse = z
  .object({
    accountId: z.string(),
    balance: z.number(),
  })
  .openapi({
    ref: "AccountBalanceResponse",
  });
export type AccountBalanceResponse = z.infer<typeof AccountBalanceResponse>;
