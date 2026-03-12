import { randomUUID } from "node:crypto";
import type Redis from "ioredis";
import { redis } from "./redis";

export interface AuthLog {
  id: string;
  type: "login" | "logout";
  method?: "TOTP" | "PASSKEY";
  credentialName?: string;
  time: string;
  ip: string;
  userAgent: string;
  success: boolean;
  message?: string;
  ipLocation?: string;
}

export class AuthLogManager {
  private redis: Redis;
  private logsIndexKey = "fn_knock:auth_logs:index";

  constructor() {
    this.redis = redis;
  }

  /**
   * Records an authentication log with a 30-day TTL.
   */
  async recordLog(logData: Omit<AuthLog, "id" | "time">): Promise<void> {
    const id = randomUUID();
    const time = new Date().toISOString();
    const timestamp = Date.now();
    
    // Attempt to get IP location
    let ipLocationStr = "";
    try {
      const { ipLocationService } = await import("./ip-location");
      const ipAddr = logData.ip == '::1' ? '127.0.0.1' : logData.ip
      const ipInfo = await ipLocationService.getIpLocation(ipAddr);
      if (ipInfo) {
        ipLocationStr = ipInfo.raw;
      }
    } catch (err) {
      console.error("Failed to query IP location:", err);
    }
    
    const log: AuthLog = {
      id,
      time,
      ...logData,
    };

    if (ipLocationStr) {
      log.ipLocation = ipLocationStr;
    }

    const logKey = `fn_knock:auth_log_data:${id}`;
    const ttlSeconds = 30 * 24 * 3600; // 30 days

    const pipeline = this.redis.pipeline();
    pipeline.set(logKey, JSON.stringify(log), "EX", ttlSeconds);
    pipeline.zadd(this.logsIndexKey, timestamp, id);
    await pipeline.exec();
  }

  /**
   * Retrieves paginated logs, cleaning up expired ones from the index.
   */
  async getLogs(page: number = 1, limit: number = 50, search: string = ""): Promise<{ logs: AuthLog[], total: number }> {
    const safePage = Math.max(1, Math.floor(page || 1));
    const safeLimit = Math.max(1, Math.min(Math.floor(limit || 50), 200));
    const keyword = search.trim().toLowerCase();

    if (!keyword) {
      const total = await this.redis.zcard(this.logsIndexKey);
      if (total === 0) return { logs: [], total: 0 };

      const start = (safePage - 1) * safeLimit;
      const end = start + safeLimit - 1;
      const ids = await this.redis.zrevrange(this.logsIndexKey, start, end);
      if (ids.length === 0) return { logs: [], total };

      const { logs, staleIds } = await this.getLogsByIds(ids);
      if (staleIds.length > 0) {
        this.redis.zrem(this.logsIndexKey, ...staleIds).catch((e) => {
          console.error("Failed to clean up expired auth logs from index:", e);
        });
      }
      return { logs, total };
    }

    const chunkSize = Math.max(200, safeLimit * 4);
    const pageStart = (safePage - 1) * safeLimit;
    let matchedTotal = 0;
    let offset = 0;
    const resultLogs: AuthLog[] = [];
    const staleIds: string[] = [];

    while (true) {
      const ids = await this.redis.zrevrange(this.logsIndexKey, offset, offset + chunkSize - 1);
      if (ids.length === 0) break;
      offset += ids.length;

      const { logs, staleIds: stale } = await this.getLogsByIds(ids);
      if (stale.length > 0) staleIds.push(...stale);

      for (const log of logs) {
        if (!this.matchesSearch(log, keyword)) continue;
        if (matchedTotal >= pageStart && resultLogs.length < safeLimit) {
          resultLogs.push(log);
        }
        matchedTotal += 1;
      }
    }

    if (staleIds.length > 0) {
      this.redis.zrem(this.logsIndexKey, ...staleIds).catch((e) => {
        console.error("Failed to clean up expired auth logs from index:", e);
      });
    }

    return { logs: resultLogs, total: matchedTotal };
  }

  private matchesSearch(log: AuthLog, keyword: string): boolean {
    return (
      log.ip.toLowerCase().includes(keyword) ||
      log.userAgent.toLowerCase().includes(keyword) ||
      (!!log.credentialName && log.credentialName.toLowerCase().includes(keyword)) ||
      log.type.toLowerCase().includes(keyword) ||
      (!!log.method && log.method.toLowerCase().includes(keyword))
    );
  }

  private async getLogsByIds(ids: string[]): Promise<{ logs: AuthLog[]; staleIds: string[] }> {
    if (ids.length === 0) return { logs: [], staleIds: [] };
    const logKeys = ids.map((id) => `fn_knock:auth_log_data:${id}`);
    const rawLogs = await this.redis.mget(logKeys);
    const logs: AuthLog[] = [];
    const staleIds: string[] = [];

    rawLogs.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (raw === null) {
        staleIds.push(id);
        return;
      }
      try {
        logs.push(JSON.parse(raw) as AuthLog);
      } catch {
        staleIds.push(id);
      }
    });

    return { logs, staleIds };
  }

  async listLogsByRange(fromMs: number, toMs: number): Promise<Array<{ id: string; timestamp: number; log: AuthLog }>> {
    const pairs = await this.redis.zrangebyscore(this.logsIndexKey, fromMs, toMs, "WITHSCORES");
    if (!pairs.length) return [];

    const ids: string[] = [];
    const timestamps: number[] = [];
    for (let i = 0; i < pairs.length; i += 2) {
      const id = pairs[i];
      const score = Number(pairs[i + 1]);
      if (!id || !Number.isFinite(score)) continue;
      ids.push(id);
      timestamps.push(score);
    }

    if (!ids.length) return [];

    const logKeys = ids.map((id) => `fn_knock:auth_log_data:${id}`);
    const rawLogs = await this.redis.mget(logKeys);
    const expiredIds: string[] = [];
    const items: Array<{ id: string; timestamp: number; log: AuthLog }> = [];

    rawLogs.forEach((raw, index) => {
      const id = ids[index];
      const timestamp = timestamps[index];
      if (!id || typeof timestamp !== "number" || !Number.isFinite(timestamp)) return;
      if (raw === null) {
        expiredIds.push(id);
        return;
      }
      try {
        const parsed = JSON.parse(raw as string) as AuthLog;
        items.push({ id, timestamp, log: parsed });
      } catch {
        expiredIds.push(id);
      }
    });

    if (expiredIds.length > 0) {
      this.redis.zrem(this.logsIndexKey, ...expiredIds).catch((e) => {
        console.error("Failed to clean up expired auth logs from index:", e);
      });
    }

    return items;
  }

  /**
   * Deletes multiple logs by their IDs.
   */
  async deleteLogs(ids: string[]): Promise<void> {
    if (ids.length === 0) return;
    const logKeys = ids.map((id) => `fn_knock:auth_log_data:${id}`);
    
    // Delete data keys
    await this.redis.del(...logKeys);
    
    // Remove from index
    await this.redis.zrem(this.logsIndexKey, ...ids);
  }
}

export const authLogManager = new AuthLogManager();
