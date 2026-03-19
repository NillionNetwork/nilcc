import { describeRoute } from "hono-openapi";
import { resolver } from "hono-openapi/zod";
import { StatusCodes } from "http-status-codes";
import { microdollarsToUsd } from "#/common/nil";
import { OpenApiSpecCommonErrorResponses } from "#/common/openapi";
import { PathsV1 } from "#/common/paths";
import type { ControllerOptions } from "#/common/types";
import { payloadValidator } from "#/common/zod-utils";
import {
  ChallengeRequest,
  ChallengeResponse,
  LoginRequest,
  LoginResponse,
} from "./auth.dto";
import { AuthenticationFailed } from "./auth.service";

export function challenge(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.auth.challenge,
    describeRoute({
      tags: ["auth"],
      summary: "Request a sign-in challenge",
      description:
        "Returns a message to sign with your wallet for authentication.",
      responses: {
        200: {
          description: "Challenge created successfully",
          content: {
            "application/json": {
              schema: resolver(ChallengeResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    payloadValidator(ChallengeRequest),
    async (c) => {
      const payload = c.req.valid("json");
      const result = await bindings.services.auth.createChallenge(
        bindings,
        payload.walletAddress,
      );
      return c.json(result);
    },
  );
}

export function login(options: ControllerOptions) {
  const { app, bindings } = options;
  app.post(
    PathsV1.auth.login,
    describeRoute({
      tags: ["auth"],
      summary: "Authenticate with a signed challenge",
      description:
        "Verifies the wallet signature and returns a JWT token. Creates an account on first sign-in.",
      responses: {
        200: {
          description: "Login successful",
          content: {
            "application/json": {
              schema: resolver(LoginResponse),
            },
          },
        },
        ...OpenApiSpecCommonErrorResponses,
      },
    }),
    payloadValidator(LoginRequest),
    async (c) => {
      const payload = c.req.valid("json");
      try {
        const result = await bindings.services.auth.verifyAndLogin(
          bindings,
          payload.message,
          payload.signature as `0x${string}`,
        );

        const account = await bindings.services.account.findByWalletAddress(
          bindings,
          (await bindings.services.auth.verifyToken(bindings, result.token))
            .wallet,
        );

        return c.json({
          token: result.token,
          expiresAt: result.expiresAt.toISOString(),
          account: {
            accountId: account?.id,
            walletAddress: account?.walletAddress,
            balance: account ? microdollarsToUsd(account.balance) : 0,
          },
        });
      } catch (e) {
        if (e instanceof AuthenticationFailed) {
          return c.json(
            { error: e.reason, kind: "AUTHENTICATION_FAILED" },
            StatusCodes.UNAUTHORIZED,
          );
        }
        throw e;
      }
    },
  );
}
