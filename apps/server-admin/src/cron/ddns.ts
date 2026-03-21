import type { Elysia } from "elysia";
import { cron } from "@elysiajs/cron";
import { runAutomaticDDNSCheck } from "../lib/ddns/auto-check";

export const registerDDNSCron = (app: Elysia) => {
  const pattern = process.env.DDNS_CRON || "*/5 * * * *";

  app.use(
    cron({
      name: "ddns-update",
      pattern,
      async run() {
        await runAutomaticDDNSCheck({
          trigger: "cron",
          emitSkipLog: true,
          emitNoopLog: true,
        });
      },
    })
  );

  return app;
};
