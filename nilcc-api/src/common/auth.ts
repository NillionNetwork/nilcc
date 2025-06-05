import type { Context, Next } from "hono";

export function apiKey(apiKey: string) {
  return async (c: Context, next: Next) => {
    const requestApiKey = c.req.header("x-api-key");
    if (!requestApiKey || requestApiKey !== apiKey) {
      return c.json({ error: "Unauthorized" }, 401);
    }
    await next();
    return;
  };
}
