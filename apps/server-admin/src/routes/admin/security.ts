import { Elysia, t } from "elysia";
import {
  FN_EVENT_AUTH_LOGIN_FAILURE,
  FN_EVENT_SECURITY_SCANNER_BLOCKED,
} from "../../lib/system-events/constants";
import { systemEventManager } from "../../lib/system-events/manager";
import { wafLogStore } from "../../lib/waf/log-store";
import { withRouteDoc } from "../../lib/openapi";
import { clamp, parseIntSafe } from "./shared";

const buildCountSeries = (
  timestamps: number[],
  fromMs: number,
  toMs: number,
  bucketCount: number,
) => {
  const span = Math.max(1, toMs - fromMs);
  const step = Math.max(1, Math.ceil(span / bucketCount));
  const buckets = Array.from({ length: bucketCount }, () => 0);
  for (const ts of timestamps) {
    if (!Number.isFinite(ts)) continue;
    const idx = Math.min(
      bucketCount - 1,
      Math.max(0, Math.floor((ts - fromMs) / step)),
    );
    const current = buckets[idx] ?? 0;
    buckets[idx] = current + 1;
  }
  return buckets.map(
    (count, index) => [fromMs + index * step, count] as [number, number],
  );
};

export const adminSecurityRoutes = new Elysia().get(
  "/security/overview",
  async ({ query }) => {
    const rangeSec = clamp(
      parseIntSafe(query.rangeSec, 3600),
      60,
      30 * 24 * 3600,
    );
    const nowMs = Date.now();
    const fromMs = nowMs - rangeSec * 1000;
    const bucketCount = Math.min(48, Math.max(12, Math.round(rangeSec / 900)));
    const events = await systemEventManager.listByRange({
      fromMs,
      toMs: nowMs,
      types: [FN_EVENT_AUTH_LOGIN_FAILURE, FN_EVENT_SECURITY_SCANNER_BLOCKED],
    });
    const wafOverview = await wafLogStore.getRangeSeries({
      fromMs,
      toMs: nowMs,
      bucketCount,
    });
    const failedTimestamps = events
      .filter((item) => item.event.type === FN_EVENT_AUTH_LOGIN_FAILURE)
      .map((item) => item.timestamp);
    const blockedTimestamps = events
      .filter((item) => item.event.type === FN_EVENT_SECURITY_SCANNER_BLOCKED)
      .map((item) => item.timestamp);

    return {
      success: true,
      data: {
        rangeSec,
        totals: {
          failedLogins: failedTimestamps.length,
          blockedScanners: blockedTimestamps.length,
          wafEvents: wafOverview.total,
        },
        series: {
          failedLogins: buildCountSeries(
            failedTimestamps,
            fromMs,
            nowMs,
            bucketCount,
          ),
          blockedScanners: buildCountSeries(
            blockedTimestamps,
            fromMs,
            nowMs,
            bucketCount,
          ),
          wafEvents: wafOverview.series,
        },
      },
    };
  },
  withRouteDoc("获取安全概览统计", {
    query: t.Object({
      rangeSec: t.Optional(t.String()),
    }),
  }),
);
