import tsconfigPaths from "vite-tsconfig-paths";
import { defineConfig } from "vitest/config";

export default defineConfig({
  plugins: [tsconfigPaths()],
  test: {
    globalSetup: "./e2e-tests/fixture/global-setup.ts",
    testTimeout: 0,
    env: {
      DEBUG: "@nillion*",
    },
    coverage: {
      reporter: ["text", "json-summary", "json"],
      reportOnFailure: true,
    },
    // run tests sequentially
    threads: false,
    isolate: false,
    hookTimeout: 30000,
    teardownTimeout: 30000,
  },
});
