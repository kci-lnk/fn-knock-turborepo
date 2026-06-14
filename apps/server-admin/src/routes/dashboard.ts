import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";
import {
  normalizeTrafficHost,
  trafficMetricsManager,
} from "../lib/traffic-metrics";
import { routeDoc, withRouteDoc } from "../lib/openapi";
import { configManager } from "../lib/redis";
import { createRequestTranslator } from "../lib/i18n";

const getDashboardRouteTranslator = async (request: Request) => {
  const config = await configManager.getConfig();
  return createRequestTranslator(request, config.locale);
};

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const toUnixSeconds = (ms = Date.now()) => Math.floor(ms / 1000);

const clamp = (n: number, min: number, max: number) =>
  Math.max(min, Math.min(max, n));

const toIsoStringSafe = (value: unknown) => {
  const date = new Date(String(value ?? ""));
  if (!Number.isFinite(date.getTime())) return "";
  return date.toISOString();
};

const normalizeActiveIPItem = (item: {
  ip?: unknown;
  last_seen_at?: unknown;
  active_conns?: unknown;
}) => {
  const ip = String(item?.ip ?? "").trim();
  const lastSeenAt = toIsoStringSafe(item?.last_seen_at);
  const activeConns = Number(item?.active_conns ?? 0);
  if (!ip || !lastSeenAt) return null;
  return {
    ip,
    last_seen_at: lastSeenAt,
    active_conns:
      Number.isFinite(activeConns) && activeConns > 0
        ? Math.floor(activeConns)
        : 0,
  };
};

const normalizeActiveIPWindowSeconds = (value: unknown) => {
  const seconds = Number(value ?? 120);
  if (!Number.isFinite(seconds)) return 120;
  return Math.max(1, Math.floor(seconds));
};

const normalizeActiveIPCount = (value: unknown) => {
  const count = Number(value ?? 0);
  if (!Number.isFinite(count) || count < 0) return 0;
  return Math.floor(count);
};

const pointsToBpsData = (
  points: { ts: number; delta: number }[],
  rangeSec: number,
) => {
  if (!points.length) return [];
  const sorted = points.slice().sort((a, b) => a.ts - b.ts);

  // Cap point count to keep ECharts and network payloads responsive.
  const MAX_POINTS = 300;

  // Use raw points directly when the range is already small enough.
  if (sorted.length <= MAX_POINTS) {
    let lastTs = sorted[0]!.ts - 1;
    return sorted.map((p) => {
      const dt = Math.max(1, p.ts - lastTs);
      lastTs = p.ts;
      const bps = Math.round((p.delta / dt) * 1000) / 1000;
      return [p.ts * 1000, bps] as const;
    });
  }

  // Downsample by splitting the range into MAX_POINTS buckets.
  const bucketSec = Math.max(1, Math.ceil(rangeSec / MAX_POINTS));
  const result: [number, number][] = [];

  let currentBucketTs = Math.floor(sorted[0]!.ts / bucketSec) * bucketSec;
  let currentBucketDelta = 0;
  let hasData = false;

  for (const p of sorted) {
    const bucket = Math.floor(p.ts / bucketSec) * bucketSec;
    if (bucket !== currentBucketTs) {
      if (hasData) {
        // Calculate average bytes per second in this bucket.
        const bps = Math.round((currentBucketDelta / bucketSec) * 1000) / 1000;
        result.push([currentBucketTs * 1000, bps]);
      }
      currentBucketTs = bucket;
      currentBucketDelta = 0;
      hasData = true;
    } else {
      hasData = true;
    }
    currentBucketDelta += p.delta;
  }

  // Flush the last bucket.
  if (hasData) {
    const bps = Math.round((currentBucketDelta / bucketSec) * 1000) / 1000;
    result.push([currentBucketTs * 1000, bps]);
  }

  return result;
};

const buildRealtimePayload = async () => {
  const resp = await goBackend.getTrafficStats();
  if (!resp.success || !resp.data) return null;
  return {
    total_in: resp.data.total_in,
    total_out: resp.data.total_out,
    active_conns: resp.data.active_conns,
    error_5xx: resp.data.error_5xx,
    by_host: (resp.data.by_host ?? [])
      .map((item) => ({
        host: normalizeTrafficHost(item.host),
        total_in: Number(item.total_in ?? 0),
        total_out: Number(item.total_out ?? 0),
        error_5xx: Number(item.error_5xx ?? 0),
        active_ip_count: normalizeActiveIPCount(item.active_ip_count),
      }))
      .filter((item) => item.host),
    timestamp: Date.now(),
  };
};

export const dashboardRoutes = new Elysia({
  prefix: "/api/admin/dashboard",
  tags: ["Dashboard"],
})
  .get(
    "/stats",
    async ({ query, request }) => {
      const { t } = await getDashboardRouteTranslator(request);
      const userId = query.userId || process.env.TRAFFIC_USER_ID || "global";
      const host = normalizeTrafficHost(query.host);
      const rangeSec = clamp(
        parseIntSafe(query.rangeSec, 3600),
        60,
        30 * 24 * 3600,
      );

      const nowSec = toUnixSeconds();
      const fromSec = nowSec - rangeSec;

      const [
        inPoints,
        outPoints,
        inSum,
        outSum,
        err5xxSum,
        err5xx1d,
        err5xx1w,
      ] = await Promise.all([
        trafficMetricsManager.listTrafficPoints(
          userId,
          "in",
          fromSec,
          nowSec,
          host,
        ),
        trafficMetricsManager.listTrafficPoints(
          userId,
          "out",
          fromSec,
          nowSec,
          host,
        ),
        trafficMetricsManager.sumTrafficDelta(
          userId,
          "in",
          fromSec,
          nowSec,
          host,
        ),
        trafficMetricsManager.sumTrafficDelta(
          userId,
          "out",
          fromSec,
          nowSec,
          host,
        ),
        trafficMetricsManager.sum5xxDelta(userId, fromSec, nowSec, host),
        trafficMetricsManager.sum5xxDelta(
          userId,
          nowSec - 24 * 3600,
          nowSec,
          host,
        ),
        trafficMetricsManager.sum5xxDelta(
          userId,
          nowSec - 7 * 24 * 3600,
          nowSec,
          host,
        ),
      ]);

      const trafficEcharts = {
        tooltip: { trigger: "axis" },
        legend: {
          data: [
            t("server.dashboard.inbound"),
            t("server.dashboard.outbound"),
          ],
        },
        xAxis: { type: "time" },
        yAxis: { type: "value" },
        series: [
          {
            name: t("server.dashboard.inbound"),
            type: "line",
            showSymbol: false,
            data: pointsToBpsData(inPoints, rangeSec),
          },
          {
            name: t("server.dashboard.outbound"),
            type: "line",
            showSymbol: false,
            data: pointsToBpsData(outPoints, rangeSec),
          },
        ],
      };

      let current: any = null;
      try {
        const resp = await goBackend.getTrafficStats();
        if (resp.success && resp.data) {
          current = resp.data;
          if (host) {
            current =
              (resp.data.by_host ?? []).find(
                (item) => normalizeTrafficHost(item.host) === host,
              ) ?? null;
          }
        }
      } catch {}

      return {
        success: true,
        data: {
          rangeSec,
          now: {
            online: current?.active_conns ?? null,
            error5xxTotal: current?.error_5xx ?? null,
          },
          totals: {
            inBytes: inSum,
            outBytes: outSum,
            error5xx: err5xxSum,
          },
          errors: {
            error5xx1d: err5xx1d,
            error5xx1w: err5xx1w,
          },
          traffic: {
            echarts: trafficEcharts,
          },
        },
      };
    },
    withRouteDoc("获取首页统计面板数据", {
      query: t.Object({
        rangeSec: t.Optional(t.String()),
        userId: t.Optional(t.String()),
        host: t.Optional(t.String()),
      }),
    }),
  )
  .get(
    "/realtime",
    async ({ set, request }) => {
      const { t } = await getDashboardRouteTranslator(request);
      try {
        const payload = await buildRealtimePayload();
        if (!payload) {
          set.status = 502;
          return {
            success: false,
            message: t("server.dashboard.upstreamUnavailable"),
          };
        }
        return { success: true, data: payload };
      } catch {
        set.status = 502;
        return {
          success: false,
          message: t("server.dashboard.upstreamUnavailable"),
        };
      }
    },
    routeDoc("获取实时流量快照"),
  )
  .get(
    "/active-ips",
    async ({ query, set, request }) => {
      const { t } = await getDashboardRouteTranslator(request);
      const host = normalizeTrafficHost(query.host);
      if (!host) {
        set.status = 400;
        return { success: false, message: t("server.dashboard.hostRequired") };
      }

      try {
        const resp = await goBackend.getHostActiveIPs(host);
        if (!resp.success || !resp.data) {
          set.status = resp.code && resp.code >= 400 ? resp.code : 502;
          return {
            success: false,
            message:
              resp.message || t("server.dashboard.upstreamUnavailable"),
          };
        }

        const items = (resp.data.items ?? [])
          .map(normalizeActiveIPItem)
          .filter((item): item is NonNullable<typeof item> => Boolean(item))
          .sort((a, b) => {
            const diff =
              new Date(b.last_seen_at).getTime() -
              new Date(a.last_seen_at).getTime();
            if (diff !== 0) return diff;
            return a.ip.localeCompare(b.ip);
          });

        return {
          success: true,
          data: {
            host: normalizeTrafficHost(resp.data.host) || host,
            window_seconds: normalizeActiveIPWindowSeconds(
              resp.data.window_seconds,
            ),
            items,
            timestamp: Date.now(),
          },
        };
      } catch {
        set.status = 502;
        return {
          success: false,
          message: t("server.dashboard.upstreamUnavailable"),
        };
      }
    },
    withRouteDoc("获取子域名活跃 IP", {
      query: t.Object({
        host: t.String(),
      }),
    }),
  );
