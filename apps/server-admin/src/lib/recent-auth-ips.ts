import type Redis from "ioredis";
import { redis } from "./redis";

class RecentAuthIPsManager {
  private readonly zsetKey = "fn_knock:recent_auth_ips:zset";
  private readonly ttlSeconds = 30 * 24 * 3600;
  private r: Redis;

  constructor() {
    this.r = redis;
  }

  async recordVerified(ip: string): Promise<void> {
    const now = Math.floor(Date.now() / 1000);
    const expireAt = now + this.ttlSeconds;
    const pipeline = this.r.pipeline();
    pipeline.zadd(this.zsetKey, expireAt, ip);
    pipeline.zremrangebyscore(this.zsetKey, 0, now);
    await pipeline.exec();
  }

  async isActive(ip: string): Promise<boolean> {
    const now = Math.floor(Date.now() / 1000);
    const score = await this.r.zscore(this.zsetKey, ip);
    if (score === null) return false;
    return Number(score) > now;
  }

  async listActive(limit = 1000): Promise<string[]> {
    const now = Math.floor(Date.now() / 1000);
    return await this.r.zrangebyscore(this.zsetKey, now + 1, "+inf", "LIMIT", 0, limit);
  }

  async cleanupExpired(): Promise<number> {
    const now = Math.floor(Date.now() / 1000);
    return await this.r.zremrangebyscore(this.zsetKey, 0, now);
  }
}

export const recentAuthIPsManager = new RecentAuthIPsManager();
