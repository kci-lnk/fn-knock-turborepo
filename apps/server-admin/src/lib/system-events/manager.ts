import { randomBytes } from "node:crypto";
import { configManager } from "../redis";
import { getDefaultSystemEventLevel, isSystemEventTypeEnabled } from "./rules";
import { redisSystemEventStore } from "./redis-store";
import type { SystemEventStore } from "./store";
import type {
  SystemEventEnvelope,
  SystemEventListQuery,
  SystemEventListResult,
  SystemEventPublishInput,
  SystemEventRangeItem,
} from "./types";
import {
  isSystemEventLevel,
  isSystemEventSource,
  isSystemEventSubjectKind,
  isSystemEventType,
  type SystemEventType,
} from "./constants";

export class SystemEventManager {
  constructor(
    private readonly store: SystemEventStore = redisSystemEventStore,
  ) {}

  async publish<T extends SystemEventType>(
    input: SystemEventPublishInput<T>,
  ): Promise<SystemEventEnvelope<T> | null> {
    if (!isSystemEventType(input.type)) {
      throw new Error(`Unsupported system event type: ${input.type}`);
    }
    if (!isSystemEventSource(input.source)) {
      throw new Error(`Unsupported system event source: ${input.source}`);
    }
    if (input.level && !isSystemEventLevel(input.level)) {
      throw new Error(`Unsupported system event level: ${input.level}`);
    }
    if (input.subject && !isSystemEventSubjectKind(input.subject.kind)) {
      throw new Error(
        `Unsupported system event subject kind: ${input.subject.kind}`,
      );
    }

    const config = await configManager.getConfig();
    const eventConfig = config.event_system;
    if (!eventConfig || !isSystemEventTypeEnabled(eventConfig, input.type)) {
      return null;
    }

    let acquiredDedupeKey = false;
    if (
      input.dedupe_key &&
      input.dedupe_ttl_seconds &&
      input.dedupe_ttl_seconds > 0
    ) {
      acquiredDedupeKey = await this.store.acquireDedupeKey(
        input.dedupe_key,
        input.dedupe_ttl_seconds,
      );
      if (!acquiredDedupeKey) {
        return null;
      }
    }

    const event = {
      id: `evt_${randomBytes(12).toString("hex")}`,
      type: input.type,
      source: input.source,
      level: input.level ?? getDefaultSystemEventLevel(input.type),
      happened_at: input.happened_at ?? new Date().toISOString(),
      ...(input.dedupe_key ? { dedupe_key: input.dedupe_key } : {}),
      ...(input.subject ? { subject: input.subject } : {}),
      ...(input.tags?.length ? { tags: [...input.tags] } : {}),
      payload: input.payload,
    } satisfies SystemEventEnvelope<T>;

    try {
      await this.store.append(event, {
        retentionDays: eventConfig.retention_days,
      });
    } catch (error) {
      if (acquiredDedupeKey && input.dedupe_key) {
        await this.store.releaseDedupeKey(input.dedupe_key).catch(
          (releaseError) => {
            console.error(
              `[system-events] failed to release dedupe key ${input.dedupe_key}:`,
              releaseError,
            );
          },
        );
      }
      throw error;
    }

    return event;
  }

  async publishSafely<T extends SystemEventType>(
    input: SystemEventPublishInput<T>,
  ): Promise<SystemEventEnvelope<T> | null> {
    try {
      return await this.publish(input);
    } catch (error) {
      console.error(`[system-events] failed to publish ${input.type}:`, error);
      return null;
    }
  }

  async getState<T = unknown>(key: string): Promise<T | null> {
    return this.store.getState<T>(key);
  }

  async setState(
    key: string,
    value: unknown,
    ttlSeconds?: number,
  ): Promise<boolean> {
    return this.store.setState(key, value, { ttlSeconds });
  }

  async setStateIfAbsent(
    key: string,
    value: unknown,
    ttlSeconds?: number,
  ): Promise<boolean> {
    return this.store.setState(key, value, {
      ttlSeconds,
      onlyIfAbsent: true,
    });
  }

  async deleteState(key: string): Promise<void> {
    await this.store.deleteState(key);
  }

  async list(query: SystemEventListQuery): Promise<SystemEventListResult> {
    return this.store.list(query);
  }

  async listByRange(args: {
    fromMs: number;
    toMs: number;
    types?: SystemEventType[];
  }): Promise<SystemEventRangeItem[]> {
    return this.store.listByRange(args);
  }

  async deleteMany(ids: string[]): Promise<void> {
    await this.store.deleteMany(ids);
  }

  async clearAll(): Promise<number> {
    return this.store.clearAll();
  }
}

export const systemEventManager = new SystemEventManager();
