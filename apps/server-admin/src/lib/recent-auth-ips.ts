import type Redis from "ioredis";
import { redis } from "./redis";
import { normalizeIp } from "./ip-normalize";

export interface RecentAuthIPEntry {
  ip: string;
  expiresAt: number;
  lastSeenAt: number;
  firstSeenAt: number;
  seenCount: number;
}

type RecentAuthIPDetail = {
  firstSeenAt?: number;
  lastSeenAt?: number;
  seenCount?: number;
};

class RecentAuthIPsManager {
  private readonly zsetKey = "fn_knock:recent_auth_ips:zset";
  private readonly detailsKey = "fn_knock:recent_auth_ips:details";
  private readonly ttlSeconds = 30 * 24 * 3600;
  private r: Redis;

  constructor() {
    this.r = redis;
  }

  async recordVerified(ip: string): Promise<void> {
    const normalizedIp = normalizeIp(ip);
    if (!normalizedIp) return;

    const now = Math.floor(Date.now() / 1000);
    const expireAt = now + this.ttlSeconds;
    const expiredIps = (await this.r.zrangebyscore(this.zsetKey, 0, now)).filter(
      (expiredIp) => expiredIp !== normalizedIp,
    );
    const currentDetail = await this.getDetail(normalizedIp);
    const nextDetail: RecentAuthIPDetail = {
      firstSeenAt: currentDetail?.firstSeenAt || now,
      lastSeenAt: now,
      seenCount: Math.max(1, Math.floor(currentDetail?.seenCount || 0) + 1),
    };
    const pipeline = this.r.pipeline();
    pipeline.zadd(this.zsetKey, expireAt, normalizedIp);
    pipeline.zremrangebyscore(this.zsetKey, 0, now);
    pipeline.hset(this.detailsKey, normalizedIp, JSON.stringify(nextDetail));
    if (expiredIps.length > 0) {
      pipeline.hdel(this.detailsKey, ...expiredIps);
    }
    await pipeline.exec();
  }

  async isActive(ip: string): Promise<boolean> {
    const normalizedIp = normalizeIp(ip);
    if (!normalizedIp) return false;

    const now = Math.floor(Date.now() / 1000);
    const score = await this.r.zscore(this.zsetKey, normalizedIp);
    if (score === null) return false;
    return Number(score) > now;
  }

  async listActive(limit = 1000): Promise<string[]> {
    const now = Math.floor(Date.now() / 1000);
    return await this.r.zrangebyscore(this.zsetKey, now + 1, "+inf", "LIMIT", 0, limit);
  }

  async listActiveWithScores(limit = 1000): Promise<RecentAuthIPEntry[]> {
    const now = Math.floor(Date.now() / 1000);
    const raw = await this.r.zrevrangebyscore(
      this.zsetKey,
      "+inf",
      now + 1,
      "WITHSCORES",
      "LIMIT",
      0,
      limit,
    );
    const entries: Array<{ ip: string; expiresAt: number }> = [];
    const seen = new Set<string>();

    for (let index = 0; index < raw.length; index += 2) {
      const ip = normalizeIp(raw[index] || "");
      const expiresAt = Number(raw[index + 1] || 0);
      if (!ip || !Number.isFinite(expiresAt) || seen.has(ip)) continue;
      seen.add(ip);
      entries.push({ ip, expiresAt });
    }

    if (entries.length === 0) {
      return [];
    }

    const detailValues = await this.r.hmget(
      this.detailsKey,
      ...entries.map((entry) => entry.ip),
    );

    return entries.map((entry, index) => {
      const detail = this.parseDetail(detailValues[index]);
      const fallbackLastSeenAt = Math.max(0, entry.expiresAt - this.ttlSeconds);
      const lastSeenAt = detail?.lastSeenAt || fallbackLastSeenAt;
      return {
        ip: entry.ip,
        expiresAt: entry.expiresAt,
        lastSeenAt,
        firstSeenAt: detail?.firstSeenAt || lastSeenAt,
        seenCount: Math.max(1, Math.floor(detail?.seenCount || 1)),
      };
    });
  }

  async cleanupExpired(): Promise<number> {
    const now = Math.floor(Date.now() / 1000);
    const expiredIps = await this.r.zrangebyscore(this.zsetKey, 0, now);
    const pipeline = this.r.pipeline();
    pipeline.zremrangebyscore(this.zsetKey, 0, now);
    if (expiredIps.length > 0) {
      pipeline.hdel(this.detailsKey, ...expiredIps);
    }
    const result = await pipeline.exec();
    const zremResult = result?.[0]?.[1];
    return typeof zremResult === "number" ? zremResult : 0;
  }

  private async getDetail(ip: string): Promise<RecentAuthIPDetail | null> {
    return this.parseDetail(await this.r.hget(this.detailsKey, ip));
  }

  private parseDetail(raw: string | null | undefined): RecentAuthIPDetail | null {
    if (!raw) return null;
    try {
      return JSON.parse(raw) as RecentAuthIPDetail;
    } catch {
      return null;
    }
  }
}

export const recentAuthIPsManager = new RecentAuthIPsManager();
