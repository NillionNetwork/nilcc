import { type ChildProcess, spawn } from "node:child_process";
import { afterAll, describe, expect, it } from "vitest";
import { API_URL } from "./fixture/global-setup";

let agentProcess: ChildProcess;

describe("Agent Connectivity", () => {
  afterAll(() => {
    console.log("Shutting down processes...");
    if (agentProcess) agentProcess.kill();
  });

  it("should connect to the API server and register itself", async () => {
    let agentStdOut = "";
    let agentErrorOutput = "";

    console.log("Starting agent process...");
    agentProcess = spawn(
      "../target/debug/nilcc-agent",
      ["daemon", "--config", "config/nilcc-agent-config.yaml"],
      {
        env: { ...process.env, RUST_LOG: "debug", API_ENDPOINT: API_URL },
      },
    );
    agentProcess.stdout?.on("data", (data) => {
      console.log(`[AGENT STDOUT]: ${data}`);
      agentStdOut += data.toString();
    });
    agentProcess.stderr?.on("data", (data) => {
      console.log(`[AGENT STDERR]: ${data}`);
      agentErrorOutput += data.toString();
    });

    const maxWaitTime = 15000;
    const pollInterval = 500;
    let timeWaited = 0;

    while (timeWaited < maxWaitTime) {
      if (agentStdOut.includes("Registration complete")) {
        break;
      }
      await new Promise((resolve) => setTimeout(resolve, pollInterval));
      timeWaited += pollInterval;
    }

    // TODO: Implement a proper check for the agent registration
    expect(agentStdOut).toContain("Registration complete");
    expect(agentErrorOutput).toBe("");
  });
});
