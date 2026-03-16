import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { userAuthentication } from "#/common/auth";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { PaymentListResponse } from "./payment.dto";

export function list(options: ControllerOptions) {
  const { app, bindings } = options;
  app.get(
    PathsV1.payments.list,
    describeRoute({
      tags: ["payments"],
      summary: "List payments for the authenticated account",
      description:
        "Returns a list of all on-chain payments that have been credited to this account.",
      responses: {
        200: {
          description: "Payments listed successfully",
          content: {
            "application/json": {
              schema: resolver(PaymentListResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    userAuthentication(bindings),
    async (c) => {
      const account = c.get("account");
      const payments = await bindings.services.payment.listByAccount(
        bindings,
        account.id,
      );
      return c.json(
        payments.map((p) => ({
          paymentId: p.id,
          txHash: p.txHash,
          blockNumber: p.blockNumber,
          fromAddress: p.fromAddress,
          amount: p.amount,
          depositedAmount: p.depositedAmount,
          createdAt: p.createdAt.toISOString(),
        })),
      );
    },
  );
}
