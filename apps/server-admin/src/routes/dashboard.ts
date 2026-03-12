import { Elysia, t } from "elysia";
import { goBackend } from "../lib/go-backend";
import { trafficMetricsManager } from "../lib/traffic-metrics";

const parseIntSafe = (value: string | undefined, fallback: number) => {
  const v = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(v)) return fallback;
  return v;
};

const toUnixSeconds = (ms = Date.now()) => Math.floor(ms / 1000);

const clamp = (n: number, min: number, max: number) => Math.max(min, Math.min(max, n));

const pointsToBpsData = (points: { ts: number; delta: number }[], rangeSec: number) => {
  if (!points.length) return [];
  const sorted = points.slice().sort((a, b) => a.ts - b.ts);

  // 强制最大数据点数量，防止前端 ECharts 卡死及网络传输过大
  const MAX_POINTS = 300; 

  // 如果原始数据量在安全范围内，直接计算
  if (sorted.length <= MAX_POINTS) {
    let lastTs = sorted[0]!.ts - 1;
    return sorted.map((p) => {
      const dt = Math.max(1, p.ts - lastTs);
      lastTs = p.ts;
      const bps = Math.round((p.delta / dt) * 1000) / 1000;
      return [p.ts * 1000, bps] as const;
    });
  }

  // 降采样逻辑：将时间跨度划分为 MAX_POINTS 个区块（桶）
  const bucketSec = Math.max(1, Math.ceil(rangeSec / MAX_POINTS));
  const result: [number, number][] = [];

  let currentBucketTs = Math.floor(sorted[0]!.ts / bucketSec) * bucketSec;
  let currentBucketDelta = 0;
  let hasData = false;

  for (const p of sorted) {
    const bucket = Math.floor(p.ts / bucketSec) * bucketSec;
    if (bucket !== currentBucketTs) {
      if (hasData) {
        // 计算该桶内的平均每秒速率 (BPS)
        const bps = Math.round((currentBucketDelta / bucketSec) * 1000) / 1000;
        result.push([currentBucketTs * 1000, bps]);
      }
      currentBucketTs = bucket;
      currentBucketDelta = 0; // 重置增量
      hasData = true;
    } else {
      hasData = true;
    }
    currentBucketDelta += p.delta;
  }

  // 收尾最后一个桶
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
    timestamp: Date.now(),
  };
};

export const dashboardRoutes = new Elysia({ prefix: "/api/admin/dashboard" })
  .get(
    "/stats",
    async ({ query }) => {
      const userId = query.userId || process.env.TRAFFIC_USER_ID || "global";
      const rangeSec = clamp(parseIntSafe(query.rangeSec, 3600), 60, 30 * 24 * 3600);

      const nowSec = toUnixSeconds();
      const fromSec = nowSec - rangeSec;

      const [inPoints, outPoints, inSum, outSum, err5xxSum, err5xx1d, err5xx1w] = await Promise.all([
        trafficMetricsManager.listTrafficPoints(userId, "in", fromSec, nowSec),
        trafficMetricsManager.listTrafficPoints(userId, "out", fromSec, nowSec),
        trafficMetricsManager.sumTrafficDelta(userId, "in", fromSec, nowSec),
        trafficMetricsManager.sumTrafficDelta(userId, "out", fromSec, nowSec),
        trafficMetricsManager.sum5xxDelta(userId, fromSec, nowSec),
        trafficMetricsManager.sum5xxDelta(userId, nowSec - 24 * 3600, nowSec),
        trafficMetricsManager.sum5xxDelta(userId, nowSec - 7 * 24 * 3600, nowSec),
      ]);

      const trafficEcharts = {
        tooltip: { trigger: "axis" },
        legend: { data: ["入站", "出站"] },
        xAxis: { type: "time" },
        yAxis: { type: "value" },
        series: [
          { name: "入站", type: "line", showSymbol: false, data: pointsToBpsData(inPoints, rangeSec) },
          { name: "出站", type: "line", showSymbol: false, data: pointsToBpsData(outPoints, rangeSec) },
        ],
      };

      let current: any = null;
      try {
        const resp = await goBackend.getTrafficStats();
        if (resp.success && resp.data) current = resp.data;
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
    {
      query: t.Object({
        rangeSec: t.Optional(t.String()),
        userId: t.Optional(t.String()),
      }),
    }
  )
  .get("/realtime", async ({ set }) => {
    try {
      const payload = await buildRealtimePayload();
      if (!payload) {
        set.status = 502;
        return { success: false, message: "upstream unavailable" };
      }
      return { success: true, data: payload };
    } catch {
      set.status = 502;
      return { success: false, message: "upstream unavailable" };
    }
  });
