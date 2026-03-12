import type Redis from "ioredis";
import { redis } from "./redis";

export type TrafficDirection = "in" | "out";

export type TrafficSnapshot = {
  total_in: number;
  total_out: number;
  active_conns: number;
  error_5xx: number;
};

export type TrafficDeltaPoint = {
  ts: number;
  delta: number;
};

const parseNumberSafe = (value: string | null | undefined): number | null => {
  if (value === null || value === undefined) return null;
  const n = Number(value);
  if (!Number.isFinite(n)) return null;
  return n;
};

const toUnixSeconds = (ms = Date.now()) => Math.floor(ms / 1000);

const computeDelta = (currentTotal: number, lastTotal: number | null) => {
  if (!Number.isFinite(currentTotal) || currentTotal < 0) return 0;
  if (lastTotal === null || !Number.isFinite(lastTotal) || lastTotal < 0) return currentTotal;
  if (currentTotal >= lastTotal) return currentTotal - lastTotal;
  return currentTotal;
};

const normalizeSeconds = (value: unknown, fallback: number, min: number, max: number) => {
  const n = Number(value);
  if (!Number.isFinite(n)) return fallback;
  return Math.max(min, Math.min(max, Math.floor(n)));
};

export class TrafficMetricsManager {
  private readonly r: Redis;
  private readonly keyIndexTraffic = "fn_knock:traffic:keys";
  private readonly keyIndexError5xx = "fn_knock:errors:5xx:keys";

  constructor() {
    this.r = redis;
  }

  private trafficKey(userId: string, direction: TrafficDirection) {
    return `fn_knock:traffic:${userId}:${direction}`;
  }

  private trafficLastTotalKey(userId: string, direction: TrafficDirection) {
    return `fn_knock:traffic:last:${userId}:${direction}`;
  }

  private error5xxKey(userId: string) {
    return `fn_knock:errors:${userId}:5xx`;
  }

  private error5xxLastTotalKey(userId: string) {
    return `fn_knock:errors:last:${userId}:5xx`;
  }

  async recordSnapshot(
    userId: string,
    snapshot: TrafficSnapshot,
    opts?: { nowSec?: number; keepSeconds?: number }
  ): Promise<{
    nowSec: number;
    deltaIn: number;
    deltaOut: number;
    delta5xx: number;
  }> {
    const nowSec = opts?.nowSec ?? toUnixSeconds();
    const keepSeconds = normalizeSeconds(opts?.keepSeconds, 7 * 24 * 3600, 60, 365 * 24 * 3600);
    const expireBeforeSec = nowSec - keepSeconds;

    const directionIn: TrafficDirection = "in";
    const directionOut: TrafficDirection = "out";

    const lastKeys = [
      this.trafficLastTotalKey(userId, directionIn),
      this.trafficLastTotalKey(userId, directionOut),
      this.error5xxLastTotalKey(userId),
    ];

    const [lastInRaw, lastOutRaw, last5xxRaw] = await this.r.mget(...lastKeys);
    const lastIn = parseNumberSafe(lastInRaw);
    const lastOut = parseNumberSafe(lastOutRaw);
    const last5xx = parseNumberSafe(last5xxRaw);

    const deltaIn = computeDelta(snapshot.total_in, lastIn);
    const deltaOut = computeDelta(snapshot.total_out, lastOut);
    const delta5xx = computeDelta(snapshot.error_5xx, last5xx);

    const memberIn = `${nowSec}:${deltaIn}`;
    const memberOut = `${nowSec}:${deltaOut}`;
    const member5xx = `${nowSec}:${delta5xx}`;

    const keyIn = this.trafficKey(userId, directionIn);
    const keyOut = this.trafficKey(userId, directionOut);
    const key5xx = this.error5xxKey(userId);

    const pipeline = this.r.pipeline();
    pipeline.set(this.trafficLastTotalKey(userId, directionIn), String(snapshot.total_in));
    pipeline.set(this.trafficLastTotalKey(userId, directionOut), String(snapshot.total_out));
    pipeline.set(this.error5xxLastTotalKey(userId), String(snapshot.error_5xx));

    pipeline.zadd(keyIn, nowSec, memberIn);
    pipeline.zadd(keyOut, nowSec, memberOut);
    pipeline.zadd(key5xx, nowSec, member5xx);

    pipeline.sadd(this.keyIndexTraffic, keyIn, keyOut);
    pipeline.sadd(this.keyIndexError5xx, key5xx);

    pipeline.zremrangebyscore(keyIn, 0, expireBeforeSec);
    pipeline.zremrangebyscore(keyOut, 0, expireBeforeSec);
    pipeline.zremrangebyscore(key5xx, 0, expireBeforeSec);

    await pipeline.exec();

    return { nowSec, deltaIn, deltaOut, delta5xx };
  }

  async listTrafficPoints(userId: string, direction: TrafficDirection, fromSec: number, toSec: number) {
    const key = this.trafficKey(userId, direction);
    const members = await this.r.zrangebyscore(key, fromSec, toSec);
    return this.parsePoints(members);
  }

  async list5xxPoints(userId: string, fromSec: number, toSec: number) {
    const key = this.error5xxKey(userId);
    const members = await this.r.zrangebyscore(key, fromSec, toSec);
    return this.parsePoints(members);
  }

  async sumTrafficDelta(userId: string, direction: TrafficDirection, fromSec: number, toSec: number) {
    const points = await this.listTrafficPoints(userId, direction, fromSec, toSec);
    return points.reduce((acc, p) => acc + p.delta, 0);
  }

  async sum5xxDelta(userId: string, fromSec: number, toSec: number) {
    const points = await this.list5xxPoints(userId, fromSec, toSec);
    return points.reduce((acc, p) => acc + p.delta, 0);
  }

  async cleanupExpired(keepSeconds = 7 * 24 * 3600): Promise<{ cleanedKeys: number }> {
    const nowSec = toUnixSeconds();
    const expireBeforeSec = nowSec - normalizeSeconds(keepSeconds, 7 * 24 * 3600, 60, 365 * 24 * 3600);

    const [trafficKeys, errorKeys] = await Promise.all([
      this.r.smembers(this.keyIndexTraffic),
      this.r.smembers(this.keyIndexError5xx),
    ]);

    const keys = [...new Set([...trafficKeys, ...errorKeys])].filter(Boolean);
    if (!keys.length) return { cleanedKeys: 0 };

    const pipeline = this.r.pipeline();
    for (const key of keys) pipeline.zremrangebyscore(key, 0, expireBeforeSec);
    await pipeline.exec();

    return { cleanedKeys: keys.length };
  }

  buildTrafficEchartsLine(points: TrafficDeltaPoint[], opts?: { name?: string; fallbackIntervalSec?: number }) {
    const fallbackIntervalSec = normalizeSeconds(opts?.fallbackIntervalSec, 60, 1, 24 * 3600);
    let lastTs: number | null = null;

    const data = points
      .sort((a, b) => a.ts - b.ts)
      .map((p) => {
        const dt = lastTs !== null ? Math.max(1, p.ts - lastTs) : fallbackIntervalSec;
        lastTs = p.ts;
        const bps = Math.round((p.delta / dt) * 1000) / 1000;
        return [p.ts * 1000, bps] as const;
      });

    return {
      tooltip: { trigger: "axis" },
      xAxis: { type: "time" },
      yAxis: { type: "value" },
      series: [
        {
          name: opts?.name ?? "traffic",
          type: "line",
          showSymbol: false,
          data,
        },
      ],
    };
  }

  private parsePoints(members: string[]): TrafficDeltaPoint[] {
    const points: TrafficDeltaPoint[] = [];
    for (const m of members) {
      const idx = m.indexOf(":");
      if (idx <= 0) continue;
      const ts = Number(m.slice(0, idx));
      const delta = Number(m.slice(idx + 1));
      if (!Number.isFinite(ts) || !Number.isFinite(delta)) continue;
      points.push({ ts, delta });
    }
    return points;
  }
}

export const trafficMetricsManager = new TrafficMetricsManager();
