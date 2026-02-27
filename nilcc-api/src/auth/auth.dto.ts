import { z } from "zod";

export const ChallengeRequest = z
  .object({
    walletAddress: z
      .string()
      .regex(/^0x[a-fA-F0-9]{40}$/)
      .openapi({ description: "The Ethereum wallet address." }),
  })
  .openapi({ ref: "ChallengeRequest" });
export type ChallengeRequest = z.infer<typeof ChallengeRequest>;

export const ChallengeResponse = z
  .object({
    message: z
      .string()
      .openapi({ description: "The message to sign with your wallet." }),
    nonce: z
      .string()
      .openapi({ description: "The nonce used in the challenge." }),
  })
  .openapi({ ref: "ChallengeResponse" });
export type ChallengeResponse = z.infer<typeof ChallengeResponse>;

export const LoginRequest = z
  .object({
    message: z
      .string()
      .openapi({ description: "The challenge message that was signed." }),
    signature: z
      .string()
      .regex(/^0x[a-fA-F0-9]+$/)
      .openapi({ description: "The wallet signature of the message." }),
  })
  .openapi({ ref: "LoginRequest" });
export type LoginRequest = z.infer<typeof LoginRequest>;

export const LoginResponse = z
  .object({
    token: z
      .string()
      .openapi({ description: "JWT token for authenticating API requests." }),
    expiresAt: z
      .string()
      .datetime()
      .openapi({ description: "When the token expires." }),
    account: z
      .object({
        accountId: z.string(),
        walletAddress: z.string(),
        credits: z.number(),
      })
      .openapi({ description: "The authenticated account." }),
  })
  .openapi({ ref: "LoginResponse" });
export type LoginResponse = z.infer<typeof LoginResponse>;
