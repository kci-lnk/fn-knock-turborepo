import type { Elysia } from "elysia";
import { cron } from "@elysiajs/cron";
import { goBackend } from "../lib/go-backend";
import { configManager } from "../lib/redis";
import { trafficMetricsManager } from "../lib/traffic-metrics";

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

export const registerTrafficCollectCron = (app: Elysia) => {
  const pattern = process.env.TRAFFIC_COLLECT_CRON || "*/30 * * * * *";
  const userId = process.env.TRAFFIC_USER_ID || "global";
  const keepSeconds = Math.max(60, Math.min(365 * 24 * 3600, parseIntSafe(process.env.TRAFFIC_KEEP_SECONDS, 7 * 24 * 3600)));
  const lockTtlSeconds = Math.max(1, Math.min(3600, parseIntSafe(process.env.TRAFFIC_COLLECT_LOCK_TTL, 1 * 60)));

  app.use(
    cron({
      name: "traffic-collect",
      pattern,
      async run() {
        const acquired = await configManager.setLockIfNotExists("traffic-collect", lockTtlSeconds);
        if (!acquired) return;

        try {
          const resp = await goBackend.getTrafficStats();
          if (!resp.success || !resp.data) return;
          await trafficMetricsManager.recordSnapshot(userId, resp.data, { keepSeconds });
        } catch (e: any) {
          console.error("[traffic][cron] collect error:", e?.message || String(e));
        }
      },
    })
  );

  return app;
};

export const registerTrafficCleanupCron = (app: Elysia) => {
  const pattern = process.env.TRAFFIC_CLEANUP_CRON || "0 * * * *";
  const keepSeconds = Math.max(60, Math.min(365 * 24 * 3600, parseIntSafe(process.env.TRAFFIC_KEEP_SECONDS, 7 * 24 * 3600)));
  const lockTtlSeconds = Math.max(30, Math.min(3600, parseIntSafe(process.env.TRAFFIC_CLEANUP_LOCK_TTL, 300)));

  app.use(
    cron({
      name: "traffic-redis-cleanup",
      pattern,
      async run() {
        const acquired = await configManager.setLockIfNotExists("traffic-cleanup", lockTtlSeconds);
        if (!acquired) return;

        try {
          await trafficMetricsManager.cleanupExpired(keepSeconds);
        } catch (e: any) {
          console.error("[traffic][cron] cleanup error:", e?.message || String(e));
        }
      },
    })
  );

  return app;
};
