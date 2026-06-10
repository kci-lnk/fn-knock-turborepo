import type { Elysia } from "elysia";
import { ddnsIntervalScheduler } from "../lib/ddns/scheduler";

export const registerDDNSCron = (app: Elysia) => {
  void ddnsIntervalScheduler.start().catch((error) => {
    console.error("[ddns][scheduler] start error:", error);
  });

  return app;
};
