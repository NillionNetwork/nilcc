import { jwtVerify, SignJWT } from "jose";
import type { Repository } from "typeorm";
import { v4 as uuidv4 } from "uuid";
import { type Address, getAddress, verifyMessage } from "viem";
import type { AppBindings } from "#/env";
import { NonceEntity } from "./nonce.entity";

const NONCE_TTL_MS = 5 * 60 * 1000; // 5 minutes

export type JwtPayload = {
  sub: string;
  wallet: string;
};

export class AuthService {
  getRepository(bindings: AppBindings): Repository<NonceEntity> {
    return bindings.dataSource.getRepository(NonceEntity);
  }

  async createChallenge(
    bindings: AppBindings,
    walletAddress: string,
  ): Promise<{ message: string; nonce: string }> {
    const normalizedAddress = getAddress(walletAddress);
    const nonce = uuidv4();
    const now = new Date();
    const expiresAt = new Date(now.getTime() + NONCE_TTL_MS);

    const repository = this.getRepository(bindings);
    await repository.save({
      id: nonce,
      walletAddress: normalizedAddress.toLowerCase(),
      expiresAt,
      createdAt: now,
    });

    const message = [
      "Sign in to nilCC",
      "",
      `Wallet: ${normalizedAddress}`,
      `Nonce: ${nonce}`,
      `Issued At: ${now.toISOString()}`,
      `Expiration Time: ${expiresAt.toISOString()}`,
    ].join("\n");

    return { message, nonce };
  }

  async verifyAndLogin(
    bindings: AppBindings,
    message: string,
    signature: `0x${string}`,
  ): Promise<{ token: string; expiresAt: Date }> {
    const walletFromMessageForVerify = this.parseWalletFromMessage(message);
    if (!walletFromMessageForVerify) {
      throw new AuthenticationFailed("invalid message format: missing wallet");
    }

    let recoveredAddress: boolean;
    try {
      recoveredAddress = await verifyMessage({
        address: walletFromMessageForVerify as Address,
        message,
        signature,
      });
    } catch {
      throw new AuthenticationFailed("invalid signature");
    }

    if (!recoveredAddress) {
      throw new AuthenticationFailed("invalid signature");
    }

    const nonce = this.parseNonceFromMessage(message);
    if (!nonce) {
      throw new AuthenticationFailed("invalid message format: missing nonce");
    }

    const repository = this.getRepository(bindings);
    const nonceEntity = await repository.findOneBy({ id: nonce });
    if (!nonceEntity) {
      throw new AuthenticationFailed("nonce not found or already used");
    }

    if (new Date() > nonceEntity.expiresAt) {
      await repository.delete({ id: nonce });
      throw new AuthenticationFailed("nonce expired");
    }

    const walletFromMessage = this.parseWalletFromMessage(message);
    if (
      !walletFromMessage ||
      walletFromMessage.toLowerCase() !==
        nonceEntity.walletAddress.toLowerCase()
    ) {
      throw new AuthenticationFailed("wallet address mismatch");
    }

    // Delete used nonce
    await repository.delete({ id: nonce });

    // Find or create account
    const account = await bindings.services.account.findOrCreateByWallet(
      bindings,
      nonceEntity.walletAddress,
    );

    // Issue JWT
    const jwtExpiresAt = new Date(
      Date.now() + bindings.config.jwtExpirationSeconds * 1000,
    );
    const secret = new TextEncoder().encode(bindings.config.jwtSecret);
    const token = await new SignJWT({
      sub: account.id,
      wallet: account.walletAddress,
    })
      .setProtectedHeader({ alg: "HS256" })
      .setIssuedAt()
      .setExpirationTime(jwtExpiresAt)
      .sign(secret);

    return { token, expiresAt: jwtExpiresAt };
  }

  async verifyToken(bindings: AppBindings, token: string): Promise<JwtPayload> {
    const secret = new TextEncoder().encode(bindings.config.jwtSecret);
    const { payload } = await jwtVerify(token, secret);
    if (!payload.sub || !payload.wallet) {
      throw new AuthenticationFailed("invalid token payload");
    }
    return {
      sub: payload.sub,
      wallet: payload.wallet as string,
    };
  }

  private parseNonceFromMessage(message: string): string | null {
    const match = message.match(/Nonce: ([a-f0-9-]+)/);
    return match ? match[1] : null;
  }

  private parseWalletFromMessage(message: string): string | null {
    const match = message.match(/Wallet: (0x[a-fA-F0-9]{40})/);
    return match ? match[1] : null;
  }
}

export class AuthenticationFailed extends Error {
  constructor(public reason: string) {
    super(`authentication failed: ${reason}`);
  }
}
