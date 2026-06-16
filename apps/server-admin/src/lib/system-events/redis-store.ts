import { redis } from "../redis";
import type {
  SystemEventEnvelope,
  SystemEventListQuery,
  SystemEventListResult,
  SystemEventRangeItem,
} from "./types";
import type { SystemEventType } from "./constants";
import type { SystemEventStore } from "./store";

const STREAM_KEY = "fn_knock:events:stream";
const INDEX_KEY = "fn_knock:events:index";
const DATA_KEY_PREFIX = "fn_knock:events:data:";
const DEDUPE_PREFIX = "fn_knock:events:dedupe:";
const STATE_PREFIX = "fn_knock:events:state:";
const STREAM_ID_PREFIX = "fn_knock:events:stream-id:";
const LIST_SCAN_CHUNK_SIZE = 200;
const CLEAR_CHUNK_SIZE = 500;
const MAX_EVENT_RETENTION_DAYS = 90;

const getDataKey = (id: string) => `${DATA_KEY_PREFIX}${id}`;
const getStreamIdKey = (id: string) => `${STREAM_ID_PREFIX}${id}`;

const normalizeTimestamp = (value: string): number => {
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? parsed : Date.now();
};

const matchesFilters = (
  event: SystemEventEnvelope,
  query: SystemEventListQuery,
): boolean => {
  if (query.type && event.type !== query.type) return false;
  if (query.level && event.level !== query.level) return false;
  if (query.source && event.source !== query.source) return false;

  const keyword = query.search?.trim().toLowerCase();
  if (!keyword) return true;

  const haystack = [
    event.id,
    event.type,
    event.source,
    event.level,
    event.happened_at,
    event.dedupe_key || "",
    event.subject?.kind || "",
    event.subject?.id || "",
    ...(event.tags ?? []),
    JSON.stringify(event.payload),
  ]
    .join(" ")
    .toLowerCase();

  return haystack.includes(keyword);
};

class RedisSystemEventStore implements SystemEventStore {
  async append(
    event: SystemEventEnvelope,
    options: { retentionDays?: number } = {},
  ): Promise<void> {
    const now = Date.now();
    const retentionDays = Math.min(
      MAX_EVENT_RETENTION_DAYS,
      Math.max(1, Math.floor(options.retentionDays ?? 30)),
    );
    const happenedAtMs = normalizeTimestamp(event.happened_at);
    const retentionMs = retentionDays * 86400 * 1000;
    const cutoffTimestamp = now - retentionMs;
    const expiresAtMs = happenedAtMs + retentionMs;
    const ttlSeconds = Math.max(1, Math.ceil((expiresAtMs - now) / 1000));
    const streamId = await redis.xadd(
      STREAM_KEY,
      "*",
      "event",
      JSON.stringify(event),
    );

    const pipeline = redis.pipeline();
    pipeline.set(getDataKey(event.id), JSON.stringify(event), "EX", ttlSeconds);
    pipeline.zadd(INDEX_KEY, happenedAtMs, event.id);
    if (streamId) {
      pipeline.set(getStreamIdKey(event.id), streamId, "EX", ttlSeconds);
    }
    pipeline.zremrangebyscore(INDEX_KEY, 0, cutoffTimestamp);
    pipeline.call("XTRIM", STREAM_KEY, "MINID", "~", `${cutoffTimestamp}-0`);

    await pipeline.exec();
  }

  async acquireDedupeKey(key: string, ttlSeconds: number): Promise<boolean> {
    const result = await (redis as any).set(
      `${DEDUPE_PREFIX}${key}`,
      "1",
      "EX",
      Math.max(1, Math.ceil(ttlSeconds)),
      "NX",
    );
    return result === "OK";
  }

  async releaseDedupeKey(key: string): Promise<void> {
    await redis.del(`${DEDUPE_PREFIX}${key}`);
  }

  async getState<T = unknown>(key: string): Promise<T | null> {
    const raw = await redis.get(`${STATE_PREFIX}${key}`);
    if (!raw) return null;

    try {
      return JSON.parse(raw) as T;
    } catch {
      return null;
    }
  }

  async setState(
    key: string,
    value: unknown,
    options: {
      ttlSeconds?: number;
      onlyIfAbsent?: boolean;
    } = {},
  ): Promise<boolean> {
    const storageKey = `${STATE_PREFIX}${key}`;
    const serialized = JSON.stringify(value);
    const ttlSeconds = options.ttlSeconds
      ? Math.max(1, Math.ceil(options.ttlSeconds))
      : undefined;

    if (options.onlyIfAbsent) {
      const args: Array<string | number> = [storageKey, serialized];
      if (ttlSeconds) {
        args.push("EX", ttlSeconds);
      }
      args.push("NX");
      const result = await (redis as any).set(...args);
      return result === "OK";
    }

    if (ttlSeconds) {
      await redis.set(storageKey, serialized, "EX", ttlSeconds);
      return true;
    }

    await redis.set(storageKey, serialized);
    return true;
  }

  async deleteState(key: string): Promise<void> {
    await redis.del(`${STATE_PREFIX}${key}`);
  }

  private async getEventsByIds(
    ids: string[],
  ): Promise<{ items: SystemEventRangeItem[]; staleIds: string[] }> {
    if (ids.length === 0) return { items: [], staleIds: [] };

    const rawEvents = await redis.mget(ids.map((id) => getDataKey(id)));
    const items: SystemEventRangeItem[] = [];
    const staleIds: string[] = [];

    rawEvents.forEach((raw, index) => {
      const id = ids[index];
      if (!id) return;
      if (!raw) {
        staleIds.push(id);
        return;
      }

      try {
        const event = JSON.parse(raw) as SystemEventEnvelope;
        items.push({
          id,
          timestamp: normalizeTimestamp(event.happened_at),
          event,
        });
      } catch {
        staleIds.push(id);
      }
    });

    return { items, staleIds };
  }

  async list(query: SystemEventListQuery): Promise<SystemEventListResult> {
    const safePage = Math.max(1, Math.floor(query.page || 1));
    const safeLimit = Math.max(1, Math.min(Math.floor(query.limit || 20), 100));
    const hasFilter =
      Boolean(query.search?.trim()) ||
      Boolean(query.type) ||
      Boolean(query.level) ||
      Boolean(query.source);

    if (!hasFilter) {
      const start = (safePage - 1) * safeLimit;
      while (true) {
        const total = await redis.zcard(INDEX_KEY);
        if (total === 0) return { events: [], total: 0 };

        const end = start + safeLimit - 1;
        const ids = await redis.zrevrange(INDEX_KEY, start, end);
        if (ids.length === 0) return { events: [], total };

        const { items, staleIds } = await this.getEventsByIds(ids);
        if (staleIds.length > 0) {
          await redis.zrem(INDEX_KEY, ...staleIds);
          continue;
        }

        return {
          events: items.map((item) => item.event),
          total,
        };
      }
    }

    const pageStart = (safePage - 1) * safeLimit;
    let matchedTotal = 0;
    let offset = 0;
    const events: SystemEventEnvelope[] = [];
    const staleIds: string[] = [];

    while (true) {
      const ids = await redis.zrevrange(
        INDEX_KEY,
        offset,
        offset + LIST_SCAN_CHUNK_SIZE - 1,
      );
      if (ids.length === 0) break;
      offset += ids.length;

      const batch = await this.getEventsByIds(ids);
      if (batch.staleIds.length > 0) {
        staleIds.push(...batch.staleIds);
      }

      for (const item of batch.items) {
        if (!matchesFilters(item.event, query)) continue;
        if (matchedTotal >= pageStart && events.length < safeLimit) {
          events.push(item.event);
        }
        matchedTotal += 1;
      }
    }

    if (staleIds.length > 0) {
      await redis.zrem(INDEX_KEY, ...staleIds);
    }

    return { events, total: matchedTotal };
  }

  async listByRange(args: {
    fromMs: number;
    toMs: number;
    types?: SystemEventType[];
  }): Promise<SystemEventRangeItem[]> {
    const pairs = await redis.zrangebyscore(
      INDEX_KEY,
      args.fromMs,
      args.toMs,
      "WITHSCORES",
    );
    if (!pairs.length) return [];

    const ids: string[] = [];
    for (let index = 0; index < pairs.length; index += 2) {
      const id = pairs[index];
      if (id) ids.push(id);
    }

    const { items, staleIds } = await this.getEventsByIds(ids);
    if (staleIds.length > 0) {
      await redis.zrem(INDEX_KEY, ...staleIds);
    }

    if (!args.types?.length) return items;

    const allowedTypes = new Set(args.types);
    return items.filter((item) => allowedTypes.has(item.event.type));
  }

  async deleteMany(ids: string[]): Promise<void> {
    const uniqueIds = Array.from(
      new Set(ids.map((id) => String(id).trim()).filter(Boolean)),
    );
    if (uniqueIds.length === 0) return;

    const streamIds = await redis.mget(
      uniqueIds.map((id) => getStreamIdKey(id)),
    );
    const validStreamIds = streamIds.filter((streamId): streamId is string =>
      Boolean(streamId),
    );

    const pipeline = redis.pipeline();
    pipeline.del(...uniqueIds.map((id) => getDataKey(id)));
    pipeline.del(...uniqueIds.map((id) => getStreamIdKey(id)));
    pipeline.zrem(INDEX_KEY, ...uniqueIds);
    if (validStreamIds.length > 0) {
      pipeline.xdel(STREAM_KEY, ...validStreamIds);
    }
    await pipeline.exec();
  }

  async clearAll(): Promise<number> {
    const ids = await redis.zrange(INDEX_KEY, 0, -1);
    const pipeline = redis.pipeline();

    for (let index = 0; index < ids.length; index += CLEAR_CHUNK_SIZE) {
      const batch = ids.slice(index, index + CLEAR_CHUNK_SIZE);
      pipeline.del(...batch.map((id) => getDataKey(id)));
      pipeline.del(...batch.map((id) => getStreamIdKey(id)));
    }

    pipeline.del(INDEX_KEY);
    pipeline.del(STREAM_KEY);
    await pipeline.exec();

    return ids.length;
  }
}

export const redisSystemEventStore = new RedisSystemEventStore();
