import { serve } from "@hono/node-server";
import { Command } from "commander";
import dotenv from "dotenv";
import packageJson from "../package.json";
import { buildApp } from "./app";
import { loadBindings } from "./env";
import { PaymentPoller } from "./payment/payment-poller";

export type CliOptions = {
  envFile: string;
};

async function main() {
  const program = new Command();

  program
    .name("@nillion/nilcc-api")
    .description("nilCC API server cli")
    .version(packageJson.version)
    .option("--env-file [path]", "Path to the env file (default .env)", ".env")
    .parse(process.argv);

  const options = program.opts<CliOptions>();
  console.info("! Cli options: %O", options);

  const envFilePath = options.envFile ?? ".env";
  dotenv.config({ path: envFilePath, override: true });
  const bindings = await loadBindings();
  bindings.log.info("! Enabled features: %O", bindings.config.enabledFeatures);

  bindings.log.info("Building app ...");
  const { app, metrics } = await buildApp(bindings);

  const paymentPoller = new PaymentPoller(bindings, bindings.services.payment);

  bindings.log.info("Starting servers ...");
  const appServer = serve(
    {
      fetch: app.fetch,
      port: bindings.config.httpApiPort,
    },
    () => {
      bindings.log.info(`App on :${bindings.config.httpApiPort}`);
    },
  );

  const metricsServer = serve(
    {
      fetch: metrics.fetch,
      port: bindings.config.metricsPort,
    },
    () => {
      bindings.log.info(`Metrics on :${bindings.config.metricsPort}`);
    },
  );

  let shuttingDown = false;
  const shutdown = async (): Promise<void> => {
    if (shuttingDown) {
      return;
    }
    shuttingDown = true;

    bindings.log.info(
      "Received shutdown signal. Starting graceful shutdown...",
    );

    try {
      paymentPoller.stop();
      const promises = [
        new Promise((resolve) => appServer.close(resolve)),
        new Promise((resolve) => metricsServer.close(resolve)),
        await bindings.dataSource.destroy(),
      ];

      await Promise.all(promises);

      bindings.log.info("Graceful shutdown completed. Goodbye.");
      process.exit(0);
    } catch (error) {
      console.error("Error during shutdown:", error);
      process.exit(1);
    }
  };

  process.on("SIGTERM", shutdown);
  process.on("SIGINT", shutdown);

  void paymentPoller.start().catch(async (error) => {
    bindings.log.error(error, "Failed to start payment poller");
    await shutdown();
  });
}

main().catch((error) => {
  console.error("Failed to start server:", error);
  process.exit(1);
});
