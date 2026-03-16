import * as crypto from "node:crypto";
import {
  createPublicClient,
  createWalletClient,
  defineChain,
  http,
} from "viem";
import { privateKeyToAccount } from "viem/accounts";
import { describe } from "vitest";
import type { AppBindings } from "#/env";
import { burnWithDigestEventAbi } from "#/payment/burn-contract";
import { PaymentEntity } from "#/payment/payment.entity";
import { PaymentPoller } from "#/payment/payment-poller";
import { createTestFixtureExtension } from "./fixture/it";

const RUN_ANVIL_E2E = process.env.RUN_ANVIL_E2E === "true";

const ANVIL_RPC_URL = process.env.APP_RPC_URL ?? "http://127.0.0.1:38545";
const ANVIL_CHAIN_ID = 31337;
const anvilChain = defineChain({
  id: ANVIL_CHAIN_ID,
  name: "Anvil",
  nativeCurrency: { name: "Ether", symbol: "ETH", decimals: 18 },
  rpcUrls: {
    default: { http: [ANVIL_RPC_URL] },
  },
});

const NIL_TOKEN_ADDRESS = "0x5FbDB2315678afecb367f032d93F642f64180aa3";
const BURN_WITH_DIGEST_ADDRESS = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512";

const TEST_USER_ADDRESS = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8";
const TEST_USER_PRIVATE_KEY =
  "0x59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d";

const nilTokenAbi = [
  {
    type: "function",
    name: "approve",
    stateMutability: "nonpayable",
    inputs: [
      { name: "spender", type: "address" },
      { name: "value", type: "uint256" },
    ],
    outputs: [{ name: "", type: "bool" }],
  },
] as const;

const burnWithDigestAbi = [
  {
    type: "function",
    name: "burnWithDigest",
    stateMutability: "nonpayable",
    inputs: [
      { name: "amount", type: "uint256" },
      { name: "digest", type: "bytes32" },
    ],
    outputs: [],
  },
] as const;

describe("Payment - Anvil burn detection e2e", () => {
  const { it, beforeAll, afterAll } = createTestFixtureExtension();

  beforeAll(async (_ctx) => {});
  afterAll(async (_ctx) => {});

  const runAnvilTest = RUN_ANVIL_E2E ? it : it.skip;

  runAnvilTest(
    "detects burnWithDigest events and stores the payment",
    async ({ expect, bindings, clients }) => {
      const publicClient = createPublicClient({
        chain: anvilChain,
        transport: http(ANVIL_RPC_URL),
      });

      const chainId = await publicClient.getChainId();
      expect(chainId).toBe(ANVIL_CHAIN_ID);

      bindings.config.rpcUrl = ANVIL_RPC_URL;
      bindings.config.chainId = ANVIL_CHAIN_ID;
      bindings.config.burnContractAddress = BURN_WITH_DIGEST_ADDRESS;
      bindings.config.paymentPollerIntervalMs = 200;

      const startBlock = await publicClient.getBlockNumber();
      bindings.config.paymentStartBlock = Number(startBlock) + 1;

      const account = await clients.admin
        .createAccount({
          name: "anvil-burn-e2e",
          walletAddress: TEST_USER_ADDRESS,
          credits: 0,
        })
        .submit();

      const userAccount = privateKeyToAccount(TEST_USER_PRIVATE_KEY);
      const walletClient = createWalletClient({
        account: userAccount,
        chain: anvilChain,
        transport: http(ANVIL_RPC_URL),
      });

      const burnAmount = 1_000_000n;
      const digest = `0x${crypto.randomBytes(32).toString("hex")}` as const;

      const approveTx = await walletClient.writeContract({
        address: NIL_TOKEN_ADDRESS,
        abi: nilTokenAbi,
        functionName: "approve",
        args: [BURN_WITH_DIGEST_ADDRESS, burnAmount],
      });
      await publicClient.waitForTransactionReceipt({ hash: approveTx });

      const burnTx = await walletClient.writeContract({
        address: BURN_WITH_DIGEST_ADDRESS,
        abi: burnWithDigestAbi,
        functionName: "burnWithDigest",
        args: [burnAmount, digest],
      });
      const burnReceipt = await publicClient.waitForTransactionReceipt({
        hash: burnTx,
      });

      expect(burnReceipt.status).toBe("success");

      const logs = await publicClient.getLogs({
        address: BURN_WITH_DIGEST_ADDRESS,
        event: burnWithDigestEventAbi[0],
        fromBlock: burnReceipt.blockNumber,
        toBlock: burnReceipt.blockNumber,
      });

      const emitted = logs.find((log) => log.transactionHash === burnTx);
      expect(emitted).toBeDefined();
      expect(emitted?.args.account?.toLowerCase()).toBe(
        TEST_USER_ADDRESS.toLowerCase(),
      );
      expect(emitted?.args.digest).toBe(digest);

      const poller = new PaymentPoller(bindings, bindings.services.payment);
      let payment: PaymentEntity | null = null;
      poller.start();
      try {
        payment = await waitForPayment(bindings, burnTx);
      } finally {
        poller.stop();
      }

      expect(payment).not.toBeNull();
      expect(payment?.digest).toBe(digest);
      expect(payment?.fromAddress).toBe(TEST_USER_ADDRESS.toLowerCase());

      const updatedAccount = await clients.admin
        .getAccount(account.accountId)
        .submit();
      expect(updatedAccount.credits).toBe(1000); // 1 NIL token = 1000 credits
    },
  );
});

async function waitForPayment(
  bindings: AppBindings,
  txHash: string,
): Promise<PaymentEntity | null> {
  const timeoutMs = 10_000;
  const pollIntervalMs = 200;
  const startedAt = Date.now();
  const repository = bindings.dataSource.getRepository(PaymentEntity);

  while (Date.now() - startedAt < timeoutMs) {
    const payment = await repository.findOneBy({ txHash });
    if (payment) {
      return payment;
    }
    await new Promise((resolve) => setTimeout(resolve, pollIntervalMs));
  }

  return null;
}
