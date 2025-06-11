import pino, { type Logger } from "pino";

export function createLogger(level: string, pretty: boolean): Logger {
  return pino({
    transport: pretty
      ? {
          target: "pino-pretty",
          options: {
            sync: true,
            singleLine: true,
            messageFormat: "[nilcc-api] - {msg}",
          },
        }
      : undefined,
    base: {
      pid: undefined,
    },
    timestamp: () => `,"time":"${new Date().toISOString()}"`,
    level,
  });
}
