import type Redis from "ioredis";
import { authMobilityKeys } from "./auth-mobility-keys";
import {
  buildMobilitySummary,
  limitMobilityTimelineEvents,
  nextMobilitySummaryFromEvent,
  type MobilityTimelineEvent,
  type SessionMobilitySummary,
} from "./auth-mobility-timeline";

type RedisPipeline = ReturnType<Redis["pipeline"]>;

const DEFAULT_MAX_TIMELINE_EVENTS = 100;

export class AuthMobilityTimelineStore {
  constructor(private readonly redis: Redis) {}

  queueInitializeSession(
    pipeline: RedisPipeline,
    args: {
      sessionId: string;
      loginEvent: MobilityTimelineEvent;
      ttlSeconds: number;
    },
  ): void {
    const initialEvents: MobilityTimelineEvent[] = [args.loginEvent];
    pipeline.set(
      authMobilityKeys.timeline(args.sessionId),
      JSON.stringify(initialEvents),
      "EX",
      args.ttlSeconds,
    );
    pipeline.set(
      authMobilityKeys.summary(args.sessionId),
      JSON.stringify(buildMobilitySummary(initialEvents)),
      "EX",
      args.ttlSeconds,
    );
  }

  queueClearSession(pipeline: RedisPipeline, sessionId: string): void {
    pipeline.del(authMobilityKeys.timeline(sessionId));
    pipeline.del(authMobilityKeys.summary(sessionId));
  }

  async getEvents(sessionId: string): Promise<MobilityTimelineEvent[]> {
    const raw = await this.redis.get(authMobilityKeys.timeline(sessionId));
    if (!raw) return [];

    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter(
          (event): event is MobilityTimelineEvent =>
            typeof event === "object" && event !== null,
        )
        .sort(
          (a, b) =>
            (Date.parse(a.happenedAt) || 0) - (Date.parse(b.happenedAt) || 0),
        );
    } catch {
      return [];
    }
  }

  async getSummary(sessionId: string): Promise<SessionMobilitySummary | null> {
    const raw = await this.redis.get(authMobilityKeys.summary(sessionId));
    if (!raw) return null;

    try {
      const parsed = JSON.parse(raw) as SessionMobilitySummary;
      if (
        typeof parsed === "object" &&
        parsed !== null &&
        typeof parsed.hasHistory === "boolean" &&
        typeof parsed.driftCount === "number"
      ) {
        return parsed;
      }
    } catch {
      return null;
    }

    return null;
  }

  async appendEvent(args: {
    sessionId: string;
    event: MobilityTimelineEvent;
    fallbackTtlSeconds: number | null;
    seedLoginEvent?: MobilityTimelineEvent;
    maxEvents?: number;
  }): Promise<void> {
    const timelineKey = authMobilityKeys.timeline(args.sessionId);
    const summaryKey = authMobilityKeys.summary(args.sessionId);
    const [events, storedSummary, currentTimelineTtl, currentSummaryTtl] =
      await Promise.all([
        this.getEvents(args.sessionId),
        this.getSummary(args.sessionId),
        this.redis.ttl(timelineKey),
        this.redis.ttl(summaryKey),
      ]);

    const nextEvents = limitMobilityTimelineEvents(
      events.length === 0 && args.seedLoginEvent
        ? [args.seedLoginEvent, args.event]
        : [...events, args.event],
      args.maxEvents ?? DEFAULT_MAX_TIMELINE_EVENTS,
    );
    const nextSummary = nextMobilitySummaryFromEvent(
      events,
      storedSummary,
      args.event,
      args.seedLoginEvent,
    );
    const ttlSeconds = this.resolveStorageTTL(
      currentTimelineTtl,
      currentSummaryTtl,
      args.fallbackTtlSeconds,
    );
    const pipeline = this.redis.pipeline();

    if (ttlSeconds) {
      pipeline.set(timelineKey, JSON.stringify(nextEvents), "EX", ttlSeconds);
      pipeline.set(summaryKey, JSON.stringify(nextSummary), "EX", ttlSeconds);
    } else {
      pipeline.set(timelineKey, JSON.stringify(nextEvents));
      pipeline.set(summaryKey, JSON.stringify(nextSummary));
    }

    await pipeline.exec();
  }

  private resolveStorageTTL(
    ...ttls: Array<number | null | undefined>
  ): number | null {
    const candidates = ttls.filter(
      (value): value is number =>
        typeof value === "number" && Number.isFinite(value) && value > 0,
    );
    if (candidates.length === 0) return null;
    return Math.max(...candidates);
  }
}
