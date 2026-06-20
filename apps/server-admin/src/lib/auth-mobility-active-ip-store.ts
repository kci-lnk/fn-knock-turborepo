import type Redis from "ioredis";
import { authMobilityKeys } from "./auth-mobility-keys";
import {
  parseSessionActiveIpDetail,
  type SessionActiveIpDetail,
} from "./auth-mobility-active-ip";

export class AuthMobilityActiveIpStore {
  constructor(private readonly redis: Redis) {}

  async getDetail(
    sessionId: string,
    ip: string,
  ): Promise<SessionActiveIpDetail | null> {
    const raw = await this.redis.hget(
      authMobilityKeys.activeIpDetails(sessionId),
      ip,
    );
    return parseSessionActiveIpDetail(raw);
  }

  async saveDetail(args: {
    sessionId: string;
    ip: string;
    score: number;
    detail: SessionActiveIpDetail;
    ttlSeconds: number;
  }): Promise<void> {
    const activeIpKey = authMobilityKeys.activeIpZset(args.sessionId);
    const detailKey = authMobilityKeys.activeIpDetails(args.sessionId);
    const pipeline = this.redis.pipeline();
    pipeline.zadd(activeIpKey, args.score, args.ip);
    pipeline.hset(detailKey, args.ip, JSON.stringify(args.detail));
    pipeline.expire(activeIpKey, args.ttlSeconds);
    pipeline.expire(detailKey, args.ttlSeconds);
    await pipeline.exec();
  }

  async listRecentDetails(args: {
    sessionId: string;
    since: number;
  }): Promise<SessionActiveIpDetail[]> {
    const activeIps = await this.redis.zrangebyscore(
      authMobilityKeys.activeIpZset(args.sessionId),
      args.since,
      "+inf",
    );
    if (activeIps.length === 0) return [];

    return this.readDetails(args.sessionId, activeIps);
  }

  async listAllDetails(sessionId: string): Promise<SessionActiveIpDetail[]> {
    const details = await this.redis.hgetall(
      authMobilityKeys.activeIpDetails(sessionId),
    );
    return this.sortDetails(
      Object.values(details)
        .map((raw) => parseSessionActiveIpDetail(raw))
        .filter((detail): detail is SessionActiveIpDetail => detail !== null),
    );
  }

  async collectPruneTargets(args: {
    sessionId: string;
    cutoff: number;
    keepIp?: string;
    maxEntries: number;
  }): Promise<string[]> {
    const activeIpKey = authMobilityKeys.activeIpZset(args.sessionId);
    const [expiredIps, allIps] = await Promise.all([
      this.redis.zrangebyscore(activeIpKey, 0, args.cutoff),
      this.redis.zrange(activeIpKey, 0, -1),
    ]);
    const removeIps = new Set(expiredIps);
    const remainingIps = allIps.filter((ip) => !removeIps.has(ip));
    const overflowCount = remainingIps.length - args.maxEntries;
    const normalizedKeepIp = args.keepIp || "";
    if (overflowCount > 0) {
      const overflowIps = remainingIps
        .filter((ip) => ip !== normalizedKeepIp)
        .slice(0, overflowCount);
      for (const ip of overflowIps) {
        removeIps.add(ip);
      }
    }
    return [...removeIps];
  }

  async removeIps(args: {
    sessionId: string;
    ips: string[];
  }): Promise<SessionActiveIpDetail[]> {
    if (args.ips.length === 0) return [];
    const detailKey = authMobilityKeys.activeIpDetails(args.sessionId);
    const details = await this.readDetails(args.sessionId, args.ips);
    const pipeline = this.redis.pipeline();
    pipeline.zrem(authMobilityKeys.activeIpZset(args.sessionId), ...args.ips);
    pipeline.hdel(detailKey, ...args.ips);
    await pipeline.exec();
    return details;
  }

  async expireSessionKeys(sessionId: string, ttlSeconds: number): Promise<void> {
    await Promise.all([
      this.redis.expire(authMobilityKeys.activeIpZset(sessionId), ttlSeconds),
      this.redis.expire(
        authMobilityKeys.activeIpDetails(sessionId),
        ttlSeconds,
      ),
    ]);
  }

  async clearSession(sessionId: string): Promise<void> {
    await this.redis.del(
      authMobilityKeys.activeIpZset(sessionId),
      authMobilityKeys.activeIpDetails(sessionId),
    );
  }

  private async readDetails(
    sessionId: string,
    ips: string[],
  ): Promise<SessionActiveIpDetail[]> {
    if (ips.length === 0) return [];
    const raws = await this.redis.hmget(
      authMobilityKeys.activeIpDetails(sessionId),
      ...ips,
    );
    return this.sortDetails(
      raws
        .map((raw) => parseSessionActiveIpDetail(raw))
        .filter((detail): detail is SessionActiveIpDetail => detail !== null),
    );
  }

  private sortDetails(
    details: SessionActiveIpDetail[],
  ): SessionActiveIpDetail[] {
    return details.sort((left, right) => right.lastSeenAt - left.lastSeenAt);
  }
}
