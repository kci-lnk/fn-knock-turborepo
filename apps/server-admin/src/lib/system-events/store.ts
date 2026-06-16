import type {
  SystemEventEnvelope,
  SystemEventListQuery,
  SystemEventListResult,
  SystemEventPublishInput,
  SystemEventRangeItem,
} from "./types";
import type { SystemEventType } from "./constants";

export interface SystemEventStore {
  append(
    event: SystemEventEnvelope,
    options?: { retentionDays?: number },
  ): Promise<void>;
  acquireDedupeKey(key: string, ttlSeconds: number): Promise<boolean>;
  releaseDedupeKey(key: string): Promise<void>;
  getState<T = unknown>(key: string): Promise<T | null>;
  setState(
    key: string,
    value: unknown,
    options?: {
      ttlSeconds?: number;
      onlyIfAbsent?: boolean;
    },
  ): Promise<boolean>;
  deleteState(key: string): Promise<void>;
  list(query: SystemEventListQuery): Promise<SystemEventListResult>;
  listByRange(args: {
    fromMs: number;
    toMs: number;
    types?: SystemEventType[];
  }): Promise<SystemEventRangeItem[]>;
  deleteMany(ids: string[]): Promise<void>;
  clearAll(): Promise<number>;
}

export type SystemEventIngressInput = SystemEventPublishInput;
