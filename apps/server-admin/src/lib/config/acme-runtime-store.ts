import { randomBytes } from "node:crypto";
import type Redis from "ioredis";
import {
  DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
  RedisLogBuffer,
} from "../redis-log-buffer";
import { normalizeAcmeJob, normalizeAcmeRuntimeLock } from "./app-config";
import { ACME_RUNTIME_LOCK_TTL_SECONDS } from "./env";
import { redisT } from "./messages";
import { normalizeOptionalString } from "./normalizers";
import type { AcmeJob, AcmeRuntimeLock } from "./types";

export class AcmeRuntimeStore {
  private acmeJobKey = "fn_knock:acme:job:";
  private acmeLogsKey = "fn_knock:acme:logs:";
  private acmeRuntimeLockKey = "fn_knock:acme:runtime-lock";

  constructor(private readonly redis: Redis) {}

  getAcmeRuntimeLockTtlSeconds(): number {
    return ACME_RUNTIME_LOCK_TTL_SECONDS;
  }

  private buildAcmeRuntimeLockLease(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): AcmeRuntimeLock {
    const now = new Date();
    const next = normalizeAcmeRuntimeLock({
      ...lock,
      locked: true,
      lockId:
        normalizeOptionalString(lock.lockId) || randomBytes(16).toString("hex"),
      heartbeatAt: now.toISOString(),
      expiresAt: new Date(now.getTime() + ttlSeconds * 1000).toISOString(),
    });

    return {
      ...next,
      locked: true,
    };
  }

  private async readAcmeRuntimeLockRecord(): Promise<{
    lock: AcmeRuntimeLock;
    raw: string | null;
    ttlMs: number;
  }> {
    const [raw, ttlMs] = await Promise.all([
      this.redis.get(this.acmeRuntimeLockKey),
      this.redis.pttl(this.acmeRuntimeLockKey),
    ]);

    if (!raw) {
      return { lock: { locked: false }, raw: null, ttlMs: -2 };
    }

    try {
      return {
        lock: normalizeAcmeRuntimeLock(JSON.parse(raw)),
        raw,
        ttlMs,
      };
    } catch {
      return {
        lock: { locked: false },
        raw,
        ttlMs,
      };
    }
  }

  private async clearAcmeRuntimeLockIfRawMatches(
    expectedRaw: string,
  ): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw or raw ~= ARGV[1] then
          return 0
        end
        redis.call("DEL", KEYS[1])
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      expectedRaw,
    );

    return result === 1;
  }

  private async updateAcmeRuntimeLockLeaseIfOwned(
    lockId: string,
    next: AcmeRuntimeLock,
    ttlSeconds: number,
  ): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw then
          return 0
        end
        local ok, decoded = pcall(cjson.decode, raw)
        if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
          return 0
        end
        redis.call("SET", KEYS[1], ARGV[2], "EX", tonumber(ARGV[3]))
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      lockId,
      JSON.stringify(next),
      String(ttlSeconds),
    );

    return result === 1;
  }

  private async clearAcmeRuntimeLockIfOwned(lockId: string): Promise<boolean> {
    const result = await this.redis.eval(
      `
        local raw = redis.call("GET", KEYS[1])
        if not raw then
          return 0
        end
        local ok, decoded = pcall(cjson.decode, raw)
        if not ok or type(decoded) ~= "table" or decoded["lockId"] ~= ARGV[1] then
          return 0
        end
        redis.call("DEL", KEYS[1])
        return 1
      `,
      1,
      this.acmeRuntimeLockKey,
      lockId,
    );

    return result === 1;
  }

  private isAcmeRuntimeLockExpired(
    lock: AcmeRuntimeLock,
    ttlMs: number,
  ): boolean {
    if (!lock.locked) return false;
    if (ttlMs >= 0) return false;

    const expiresAtMs = Date.parse(lock.expiresAt || "");
    if (Number.isFinite(expiresAtMs)) {
      return expiresAtMs <= Date.now();
    }

    const startedAtMs = Date.parse(lock.heartbeatAt || lock.startedAt || "");
    if (!Number.isFinite(startedAtMs)) {
      return true;
    }

    return (
      startedAtMs + this.getAcmeRuntimeLockTtlSeconds() * 1000 <= Date.now()
    );
  }

  async getAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    const { lock } = await this.readAcmeRuntimeLockRecord();
    return lock;
  }

  async tryAcquireAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    const next = this.buildAcmeRuntimeLockLease(lock, ttlSeconds);
    const result = await this.redis.set(
      this.acmeRuntimeLockKey,
      JSON.stringify(next),
      "EX",
      ttlSeconds,
      "NX",
    );
    return result === "OK" ? next : null;
  }

  async refreshAcmeRuntimeLock(
    lock: AcmeRuntimeLock,
    ttlSeconds: number = this.getAcmeRuntimeLockTtlSeconds(),
  ): Promise<AcmeRuntimeLock | null> {
    const lockId = normalizeOptionalString(lock.lockId);
    if (!lockId) return null;

    const next = this.buildAcmeRuntimeLockLease(lock, ttlSeconds);
    const updated = await this.updateAcmeRuntimeLockLeaseIfOwned(
      lockId,
      next,
      ttlSeconds,
    );
    return updated ? next : null;
  }

  async setAcmeRuntimeLock(lock: AcmeRuntimeLock): Promise<AcmeRuntimeLock> {
    const next = this.buildAcmeRuntimeLockLease(lock);
    await this.redis.set(
      this.acmeRuntimeLockKey,
      JSON.stringify(next),
      "EX",
      this.getAcmeRuntimeLockTtlSeconds(),
    );
    return next;
  }

  async releaseAcmeRuntimeLock(
    lock: AcmeRuntimeLock | string | null | undefined,
  ): Promise<boolean> {
    const lockId =
      typeof lock === "string"
        ? normalizeOptionalString(lock)
        : normalizeOptionalString(lock?.lockId);
    if (lockId) {
      return this.clearAcmeRuntimeLockIfOwned(lockId);
    }
    return false;
  }

  async clearAcmeRuntimeLock(): Promise<void> {
    await this.redis.del(this.acmeRuntimeLockKey);
  }

  async getActiveAcmeRuntimeLock(): Promise<AcmeRuntimeLock> {
    const { lock, raw, ttlMs } = await this.readAcmeRuntimeLockRecord();
    if (!lock.locked || !lock.jobId) {
      if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    if (this.isAcmeRuntimeLockExpired(lock, ttlMs)) {
      if (lock.lockId) {
        await this.releaseAcmeRuntimeLock(lock.lockId);
      } else if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    const job = await this.getAcmeJob(lock.jobId);
    if (
      !job ||
      job.status === "succeeded" ||
      job.status === "failed" ||
      job.status === "stopped"
    ) {
      if (lock.lockId) {
        await this.releaseAcmeRuntimeLock(lock.lockId);
      } else if (raw) {
        await this.clearAcmeRuntimeLockIfRawMatches(raw);
      }
      return { locked: false };
    }
    return lock;
  }

  async getActiveAcmeJobFromLock(): Promise<AcmeJob | null> {
    const lock = await this.getActiveAcmeRuntimeLock();
    if (!lock.locked || !lock.jobId) return null;
    return this.getAcmeJob(lock.jobId);
  }

  async createAcmeJob(job: AcmeJob): Promise<void> {
    const key = `${this.acmeJobKey}${job.id}`;
    const normalized = normalizeAcmeJob(job);
    if (!normalized) {
      throw new Error(redisT("acme.jobDataInvalid"));
    }
    await this.redis.set(key, JSON.stringify(normalized), "EX", 86400);
  }

  async updateAcmeJob(id: string, patch: Partial<AcmeJob>): Promise<void> {
    const key = `${this.acmeJobKey}${id}`;
    const raw = await this.redis.get(key);
    if (!raw) return;
    let obj: AcmeJob | null = null;
    try {
      obj = normalizeAcmeJob(JSON.parse(raw));
    } catch {
      return;
    }
    if (!obj) return;
    const next = { ...obj, ...patch };
    await this.redis.set(key, JSON.stringify(next), "EX", 86400);
  }

  async getAcmeJob(id: string): Promise<AcmeJob | null> {
    const raw = await this.redis.get(`${this.acmeJobKey}${id}`);
    if (!raw) return null;
    try {
      return normalizeAcmeJob(JSON.parse(raw));
    } catch {
      return null;
    }
  }

  async appendAcmeLog(jobId: string, line: string): Promise<void> {
    const key = `${this.acmeLogsKey}${jobId}`;
    const buffer = new RedisLogBuffer(this.redis, {
      key,
      ttlSeconds: 86400,
      maxLen: DEFAULT_REDIS_LOG_BUFFER_MAX_LEN,
    });
    await buffer.append([line]);
  }

  async clearAcmeLogs(jobId: string): Promise<void> {
    await this.redis.del(`${this.acmeLogsKey}${jobId}`);
  }

  async getAcmeLogs(
    jobId: string,
    limit: number = 500,
    order: "asc" | "desc" = "asc",
  ): Promise<string[]> {
    const key = `${this.acmeLogsKey}${jobId}`;
    const len = await this.redis.llen(key);
    if (len === 0) return [];
    const start = Math.max(0, len - limit);
    const arr = await this.redis.lrange(key, start, -1);
    return order === "desc" ? arr.reverse() : arr;
  }
}
