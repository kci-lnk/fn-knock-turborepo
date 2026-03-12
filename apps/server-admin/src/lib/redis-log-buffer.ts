import type Redis from "ioredis";

export const DEFAULT_REDIS_LOG_BUFFER_MAX_LEN = 1000;

type RedisLogBufferOptions = {
  key: string;
  ttlSeconds: number;
  maxLen?: number;
  seqKey?: string;
};

type PollResult = {
  cursor: number;
  reset: boolean;
  items: string[];
};

const parseCursor = (value: number | string | null | undefined): number | null => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed) || parsed < 0) return null;
  return parsed;
};

const parseRedisNumber = (value: string | null): number | null => {
  if (value === null) return null;
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : null;
};

export class RedisLogBuffer {
  private readonly redis: Redis;
  private readonly key: string;
  private readonly seqKey: string;
  private readonly ttlSeconds: number;
  private readonly maxLen: number;

  constructor(redis: Redis, options: RedisLogBufferOptions) {
    this.redis = redis;
    this.key = options.key;
    this.seqKey = options.seqKey ?? `${options.key}:seq`;
    this.ttlSeconds = options.ttlSeconds;
    this.maxLen = options.maxLen ?? DEFAULT_REDIS_LOG_BUFFER_MAX_LEN;
  }

  getMaxLen(): number {
    return this.maxLen;
  }

  async append(lines: string[]): Promise<void> {
    if (!lines.length) return;
    const pipeline = this.redis.pipeline();
    pipeline.rpush(this.key, ...lines);
    pipeline.ltrim(this.key, -this.maxLen, -1);
    pipeline.incrby(this.seqKey, lines.length);
    pipeline.expire(this.key, this.ttlSeconds);
    pipeline.expire(this.seqKey, this.ttlSeconds);
    await pipeline.exec();
  }

  async list(limit: number): Promise<string[]> {
    const safeLimit = Math.max(1, Math.min(limit, this.maxLen));
    return this.redis.lrange(this.key, -safeLimit, -1);
  }

  async clear(): Promise<void> {
    await this.redis.del(this.key, this.seqKey);
  }

  async poll(cursor?: number | string | null): Promise<PollResult> {
    const [totalLen, rawSeq] = await Promise.all([
      this.redis.llen(this.key),
      this.redis.get(this.seqKey),
    ]);
    const totalSeq = parseRedisNumber(rawSeq) ?? totalLen;
    const retainedStartSeq = Math.max(0, totalSeq - totalLen);
    const requestedCursor = parseCursor(cursor);
    const reset = requestedCursor !== null
      && (requestedCursor < retainedStartSeq || requestedCursor > totalSeq);
    const from = requestedCursor === null || reset
      ? 0
      : Math.max(0, requestedCursor - retainedStartSeq);
    const items = totalLen > 0 && from < totalLen
      ? await this.redis.lrange(this.key, from, -1)
      : [];

    return {
      cursor: totalSeq,
      reset,
      items,
    };
  }
}
