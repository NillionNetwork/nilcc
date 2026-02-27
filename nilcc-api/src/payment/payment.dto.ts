import { z } from "zod";

export const PaymentResponse = z
  .object({
    paymentId: z.string().openapi({ description: "The payment identifier." }),
    txHash: z
      .string()
      .openapi({ description: "The on-chain transaction hash." }),
    blockNumber: z
      .number()
      .openapi({ description: "The block number of the transaction." }),
    fromAddress: z
      .string()
      .openapi({ description: "The wallet address that burned tokens." }),
    amount: z
      .string()
      .openapi({ description: "The amount of tokens burned (in wei)." }),
    creditedAmount: z
      .number()
      .openapi({ description: "The number of API credits added." }),
    createdAt: z
      .string()
      .datetime()
      .openapi({ description: "When the payment was processed." }),
  })
  .openapi({ ref: "PaymentResponse" });
export type PaymentResponse = z.infer<typeof PaymentResponse>;

export const PaymentListResponse = z.array(PaymentResponse).openapi({
  ref: "PaymentListResponse",
});
export type PaymentListResponse = z.infer<typeof PaymentListResponse>;
