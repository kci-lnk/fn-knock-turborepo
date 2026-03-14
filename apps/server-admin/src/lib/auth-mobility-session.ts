import { createHash } from "node:crypto";
import type Redis from "ioredis";
import { configManager, redis, type LoginSession } from "./redis";
import { whitelistManager } from "./whitelist-manager";

type MobilitySubjectType = "proxy-session" | "fnos-token";

type MobilityBinding = {
  version: 1;
  subjectType: MobilitySubjectType;
  subjectHash: string;
  currentIp: string;
  whitelistRecordId: string;
  expireAt: number | null;
  ownerSessionId?: string;
  createdAt: string;
  lastSeenAt: string;
};

type MobilityTimelineEvent =
  | {
      version: 1;
      kind: "login";
      happenedAt: string;
      source: "login";
      toIp: string;
      toIpLocation?: string;
    }
  | {
      version: 1;
      kind: "drift";
      happenedAt: string;
      source: MobilitySubjectType;
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

export type SessionMobilitySummary = {
  hasHistory: boolean;
  driftCount: number;
  lastDriftAt: string | null;
  lastDriftSource: MobilitySubjectType | null;
};

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: MobilityTimelineEvent[];
};

type RequestIdentity = {
  sessionId: string | null;
  fnosToken: string | null;
  isFnosApp: boolean;
};

type DriftRestoreResult = {
  success: boolean;
  message?: string;
};

const PREFIX = "fn_knock:auth_mobility";
const FNOS_ACTIVITY_WINDOW_SECONDS = 12 * 3600;

const parseCookieValue = (cookieHeader: string, name: string): string | null => {
  const escaped = name.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = cookieHeader.match(new RegExp(`(?:^|;\\s*)${escaped}=([^;]+)`));
  if (!match || !match[1]) return null;
  const raw = match[1].trim().replace(/^"|"$/g, "");
  if (!raw) return null;
  try {
    return decodeURIComponent(raw);
  } catch {
    return raw;
  }
};

const toUnixSeconds = (iso?: string): number | null => {
  if (!iso) return null;
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) return null;
  return Math.floor(ms / 1000);
};

const nowSeconds = () => Math.floor(Date.now() / 1000);

export class AuthMobilitySessionManager {
  private readonly r: Redis;

  constructor() {
    this.r = redis;
  }

  inspectRequest(request: Request): RequestIdentity {
    const cookieHeader = request.headers.get("cookie") || "";
    const sessionId = parseCookieValue(cookieHeader, "x-go-reauth-proxy-session-id");
    const fnosToken = parseCookieValue(cookieHeader, "fnos-token");

    return {
      sessionId,
      fnosToken,
      isFnosApp: !!fnosToken,
    };
  }

  async registerLoginSession(args: {
    sessionId: string;
    ip: string;
    ipLocation?: string;
    whitelistRecordId: string;
    expireAt: number | null;
  }): Promise<void> {
    const ttlSeconds = this.resolveProxySessionTTL(args.expireAt);
    if (!ttlSeconds) return;

    const binding = this.buildBinding({
      subjectType: "proxy-session",
      subjectKey: args.sessionId,
      currentIp: args.ip,
      whitelistRecordId: args.whitelistRecordId,
      expireAt: args.expireAt,
      ownerSessionId: args.sessionId,
    });

    const pipeline = this.r.pipeline();
    pipeline.set(this.bindingKey("proxy-session", args.sessionId), JSON.stringify(binding), "EX", ttlSeconds);
    pipeline.set(
      this.timelineKey(args.sessionId),
      JSON.stringify([
        this.buildTimelineLoginEvent({
          ip: args.ip,
          ipLocation: args.ipLocation,
        }),
      ] satisfies MobilityTimelineEvent[]),
      "EX",
      ttlSeconds,
    );
    pipeline.sadd(this.sessionIndexKey(args.sessionId), this.bindingKey("proxy-session", args.sessionId));
    pipeline.expire(this.sessionIndexKey(args.sessionId), ttlSeconds);
    pipeline.set(this.whitelistOwnerKey(args.whitelistRecordId), args.sessionId, "EX", ttlSeconds);
    await pipeline.exec();
  }

  async syncTrustedRequest(request: Request, clientIp: string): Promise<void> {
    const identity = this.inspectRequest(request);

    if (identity.sessionId) {
      await this.refreshProxySessionBinding(identity.sessionId, clientIp);
    }

    if (identity.fnosToken) {
      await this.refreshFnosBinding(identity.fnosToken, clientIp, identity.sessionId);
    }
  }

  async tryRestoreAccess(request: Request, clientIp: string): Promise<DriftRestoreResult> {
    const identity = this.inspectRequest(request);

    if (identity.fnosToken) {
      const restored = await this.restoreFnosToken(identity.fnosToken, clientIp);
      if (restored) {
        return { success: true, message: "Authorized by fnos fingerprint session" };
      }
    }

    if (identity.sessionId) {
      const restored = await this.restoreProxySession(identity.sessionId, clientIp);
      if (restored) {
        return { success: true, message: "Authorized by session IP migration" };
      }
    }

    return { success: false };
  }

  async destroySession(sessionId: string): Promise<void> {
    const sessionKey = this.sessionIndexKey(sessionId);
    const subjectKeys = await this.r.smembers(sessionKey);
    const uniqueWhitelistRecordIds = new Set<string>();
    const proxyBinding = await this.getBinding("proxy-session", sessionId);

    if (proxyBinding?.whitelistRecordId) {
      uniqueWhitelistRecordIds.add(proxyBinding.whitelistRecordId);
    }

    for (const subjectKey of subjectKeys) {
      const binding = await this.getBindingByStorageKey(subjectKey);
      if (binding?.whitelistRecordId) {
        uniqueWhitelistRecordIds.add(binding.whitelistRecordId);
      }
    }

    const pipeline = this.r.pipeline();
    pipeline.del(this.bindingKey("proxy-session", sessionId));
    pipeline.del(this.timelineKey(sessionId));
    if (subjectKeys.length > 0) {
      pipeline.del(...subjectKeys);
    }
    pipeline.del(sessionKey);
    for (const whitelistRecordId of uniqueWhitelistRecordIds) {
      pipeline.del(this.whitelistOwnerKey(whitelistRecordId));
    }
    await pipeline.exec();

    for (const whitelistRecordId of uniqueWhitelistRecordIds) {
      await whitelistManager.removeWhiteList(whitelistRecordId);
    }
  }

  private async refreshProxySessionBinding(sessionId: string, clientIp: string): Promise<void> {
    const session = await configManager.getSession(sessionId);
    if (!session) return;

    const existing = await this.getBinding("proxy-session", sessionId);
    if (!existing) {
      return;
    }

    existing.currentIp = clientIp;
    existing.lastSeenAt = new Date().toISOString();
    existing.expireAt = toUnixSeconds(session.expiresAt);
    await this.r.set(this.bindingKey("proxy-session", sessionId), JSON.stringify(existing), "KEEPTTL");
    if (session.ip !== clientIp) {
      await configManager.updateSession(sessionId, { ip: clientIp });
    }
  }

  async getSessionMobilitySummary(sessionId: string): Promise<SessionMobilitySummary> {
    const session = await configManager.getSession(sessionId);
    const events = await this.resolveTimelineEvents(sessionId, session);
    return this.buildMobilitySummary(events);
  }

  async getSessionMobilityDetails(sessionId: string): Promise<SessionMobilityDetails> {
    const session = await configManager.getSession(sessionId);
    const events = await this.resolveTimelineEvents(sessionId, session);
    return {
      summary: this.buildMobilitySummary(events),
      events,
    };
  }

  private async refreshFnosBinding(
    fnosToken: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    const existing = await this.getBinding("fnos-token", fnosToken);
    if (!sessionId) {
      if (existing?.ownerSessionId) {
        const ownerSession = await configManager.getSession(existing.ownerSessionId);
        if (!ownerSession) return;

        const record = await whitelistManager.getRecordById(existing.whitelistRecordId);
        if (!record || record.status !== "active" || record.ip !== clientIp) return;

        const ttlSeconds = this.resolveFnosTTL(record.expireAt);
        if (!ttlSeconds) return;

        existing.currentIp = clientIp;
        existing.expireAt = record.expireAt;
        existing.lastSeenAt = new Date().toISOString();
        await this.r.set(this.bindingKey("fnos-token", fnosToken), JSON.stringify(existing), "EX", ttlSeconds);
        await this.r.sadd(this.sessionIndexKey(existing.ownerSessionId), this.bindingKey("fnos-token", fnosToken));
        await this.ensureSessionIndexTTL(
          existing.ownerSessionId,
          this.resolveProxySessionTTL(toUnixSeconds(ownerSession.expiresAt)) || ttlSeconds,
        );
        return;
      }

      const records = await whitelistManager.getActiveRecordsByIP(clientIp, "auto");
      if (records.length !== 1) return;

      const bootstrapRecord = records[0];
      if (!bootstrapRecord) return;

      const ownerSessionId = await this.r.get(this.whitelistOwnerKey(bootstrapRecord.id));
      if (!ownerSessionId) return;

      const ownerSession = await configManager.getSession(ownerSessionId);
      if (!ownerSession) return;

      const sessionTtl = this.resolveProxySessionTTL(toUnixSeconds(ownerSession.expiresAt));
      const fnosTtl = this.resolveFnosTTL(bootstrapRecord.expireAt);
      if (!sessionTtl || !fnosTtl) return;

      const binding = this.buildBinding({
        subjectType: "fnos-token",
        subjectKey: fnosToken,
        currentIp: clientIp,
        whitelistRecordId: bootstrapRecord.id,
        expireAt: bootstrapRecord.expireAt,
        ownerSessionId,
      });

      await this.r.set(this.bindingKey("fnos-token", fnosToken), JSON.stringify(binding), "EX", fnosTtl);
      await this.r.sadd(this.sessionIndexKey(ownerSessionId), this.bindingKey("fnos-token", fnosToken));
      await this.ensureSessionIndexTTL(ownerSessionId, sessionTtl);
      return;
    }

    const [session, proxyBinding] = await Promise.all([
      configManager.getSession(sessionId),
      this.getBinding("proxy-session", sessionId),
    ]);
    if (!session || !proxyBinding) return;

    const record = await whitelistManager.getRecordById(proxyBinding.whitelistRecordId);
    if (!record || record.status !== "active" || record.ip !== clientIp) return;

    if (existing?.ownerSessionId && existing.ownerSessionId !== sessionId) {
      const existingOwner = await configManager.getSession(existing.ownerSessionId);
      if (existingOwner) return;
    }

    const ttlSeconds = this.resolveFnosTTL(record.expireAt);
    if (!ttlSeconds) return;

    const binding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: clientIp,
          whitelistRecordId: record.id,
          expireAt: record.expireAt,
          ownerSessionId: sessionId,
          lastSeenAt: new Date().toISOString(),
        }
      : this.buildBinding({
          subjectType: "fnos-token",
          subjectKey: fnosToken,
          currentIp: clientIp,
          whitelistRecordId: record.id,
          expireAt: record.expireAt,
          ownerSessionId: sessionId,
        });

    await this.r.set(this.bindingKey("fnos-token", fnosToken), JSON.stringify(binding), "EX", ttlSeconds);
    await this.r.sadd(this.sessionIndexKey(sessionId), this.bindingKey("fnos-token", fnosToken));
    const sessionTtl = this.resolveProxySessionTTL(toUnixSeconds(session.expiresAt));
    if (sessionTtl) {
      await this.ensureSessionIndexTTL(sessionId, sessionTtl);
    }
  }

  private async restoreFnosToken(fnosToken: string, clientIp: string): Promise<boolean> {
    const binding = await this.getBinding("fnos-token", fnosToken);
    if (!binding) return false;

    if (!binding.ownerSessionId) return false;

    const ownerSession = await configManager.getSession(binding.ownerSessionId);
    if (!ownerSession) return false;

    const movedRecord = await whitelistManager.moveRecordToIP(binding.whitelistRecordId, clientIp);
    if (!movedRecord) return false;

    const ttlSeconds = this.resolveFnosTTL(movedRecord.expireAt);
    if (!ttlSeconds) return false;

    const previousIp = ownerSession.ip;
    const previousIpLocation = ownerSession.ipLocation;
    binding.currentIp = clientIp;
    binding.expireAt = movedRecord.expireAt;
    binding.lastSeenAt = new Date().toISOString();
    await this.r.set(this.bindingKey("fnos-token", fnosToken), JSON.stringify(binding), "EX", ttlSeconds);

    const updatedSession = await configManager.updateSession(binding.ownerSessionId, {
      ip: clientIp,
      ...(movedRecord.ipLocation ? { ipLocation: movedRecord.ipLocation } : {}),
    });
    const sessionTtl = this.resolveProxySessionTTL(toUnixSeconds(updatedSession?.expiresAt));
    if (updatedSession && sessionTtl) {
      await this.ensureSessionIndexTTL(binding.ownerSessionId, sessionTtl);
      await this.r.sadd(this.sessionIndexKey(binding.ownerSessionId), this.bindingKey("fnos-token", fnosToken));
    }

    if (previousIp !== clientIp) {
      await this.appendTimelineEvent(
        binding.ownerSessionId,
        this.buildTimelineDriftEvent({
          source: "fnos-token",
          fromIp: previousIp,
          fromIpLocation: previousIpLocation,
          toIp: clientIp,
          toIpLocation: movedRecord.ipLocation,
        }),
        sessionTtl ?? ttlSeconds,
        this.buildTimelineLoginEvent({
          ip: previousIp,
          ipLocation: previousIpLocation,
          happenedAt: ownerSession.loginTime,
        }),
      );
    }

    return true;
  }

  private async restoreProxySession(sessionId: string, clientIp: string): Promise<boolean> {
    const session = await configManager.getSession(sessionId);
    if (!session) return false;

    let binding = await this.getBinding("proxy-session", sessionId);
    if (!binding) {
      return false;
    }

    const movedRecord = await whitelistManager.moveRecordToIP(binding.whitelistRecordId, clientIp);
    if (!movedRecord) return false;

    const previousIp = session.ip;
    const previousIpLocation = session.ipLocation;
    binding.currentIp = clientIp;
    binding.expireAt = movedRecord.expireAt ?? toUnixSeconds(session.expiresAt);
    binding.lastSeenAt = new Date().toISOString();
    await this.r.set(this.bindingKey("proxy-session", sessionId), JSON.stringify(binding), "KEEPTTL");

    await configManager.updateSession(sessionId, {
      ip: clientIp,
      ...(movedRecord.ipLocation ? { ipLocation: movedRecord.ipLocation } : {}),
    });

    if (previousIp !== clientIp) {
      await this.appendTimelineEvent(
        sessionId,
        this.buildTimelineDriftEvent({
          source: "proxy-session",
          fromIp: previousIp,
          fromIpLocation: previousIpLocation,
          toIp: clientIp,
          toIpLocation: movedRecord.ipLocation,
        }),
        this.resolveProxySessionTTL(toUnixSeconds(session.expiresAt)),
        this.buildTimelineLoginEvent({
          ip: previousIp,
          ipLocation: previousIpLocation,
          happenedAt: session.loginTime,
        }),
      );
    }

    return true;
  }

  private buildBinding(args: {
    subjectType: MobilitySubjectType;
    subjectKey: string;
    currentIp: string;
    whitelistRecordId: string;
    expireAt: number | null;
    ownerSessionId?: string;
  }): MobilityBinding {
    const nowIso = new Date().toISOString();
    return {
      version: 1,
      subjectType: args.subjectType,
      subjectHash: this.hash(args.subjectType, args.subjectKey),
      currentIp: args.currentIp,
      whitelistRecordId: args.whitelistRecordId,
      expireAt: args.expireAt,
      ownerSessionId: args.ownerSessionId,
      createdAt: nowIso,
      lastSeenAt: nowIso,
    };
  }

  private buildTimelineLoginEvent(args: {
    ip: string;
    ipLocation?: string;
    happenedAt?: string;
  }): MobilityTimelineEvent {
    return {
      version: 1,
      kind: "login",
      happenedAt: args.happenedAt || new Date().toISOString(),
      source: "login",
      toIp: args.ip,
      ...(args.ipLocation ? { toIpLocation: args.ipLocation } : {}),
    };
  }

  private buildTimelineDriftEvent(args: {
    source: MobilitySubjectType;
    fromIp: string;
    fromIpLocation?: string;
    toIp: string;
    toIpLocation?: string;
  }): MobilityTimelineEvent {
    return {
      version: 1,
      kind: "drift",
      happenedAt: new Date().toISOString(),
      source: args.source,
      fromIp: args.fromIp,
      ...(args.fromIpLocation ? { fromIpLocation: args.fromIpLocation } : {}),
      toIp: args.toIp,
      ...(args.toIpLocation ? { toIpLocation: args.toIpLocation } : {}),
    };
  }

  private buildMobilitySummary(events: MobilityTimelineEvent[]): SessionMobilitySummary {
    const driftEvents = events.filter(
      (event): event is Extract<MobilityTimelineEvent, { kind: "drift" }> => event.kind === "drift",
    );
    const lastDrift = driftEvents[driftEvents.length - 1];
    return {
      hasHistory: events.length > 0,
      driftCount: driftEvents.length,
      lastDriftAt: lastDrift?.happenedAt ?? null,
      lastDriftSource: lastDrift?.source ?? null,
    };
  }

  private resolveProxySessionTTL(expireAt: number | null): number | null {
    return this.remainingSeconds(expireAt);
  }

  private resolveFnosTTL(expireAt: number | null): number | null {
    if (expireAt === null) {
      return FNOS_ACTIVITY_WINDOW_SECONDS;
    }

    const remaining = this.remainingSeconds(expireAt);
    if (!remaining) return null;
    return Math.min(remaining, FNOS_ACTIVITY_WINDOW_SECONDS);
  }

  private remainingSeconds(expireAt: number | null): number | null {
    if (expireAt === null) return null;
    const remaining = expireAt - nowSeconds();
    if (remaining <= 0) return null;
    return remaining;
  }

  private hash(subjectType: MobilitySubjectType, subjectKey: string): string {
    return createHash("sha256")
      .update(`${subjectType}:${subjectKey}`)
      .digest("hex");
  }

  private bindingKey(subjectType: MobilitySubjectType, subjectKey: string): string {
    return `${PREFIX}:binding:${subjectType}:${this.hash(subjectType, subjectKey)}`;
  }

  private timelineKey(sessionId: string): string {
    return `${PREFIX}:timeline:${sessionId}`;
  }

  private sessionIndexKey(sessionId: string): string {
    return `${PREFIX}:session:${sessionId}`;
  }

  private whitelistOwnerKey(whitelistRecordId: string): string {
    return `${PREFIX}:whitelist:${whitelistRecordId}:session`;
  }

  private async getBinding(subjectType: MobilitySubjectType, subjectKey: string): Promise<MobilityBinding | null> {
    return this.getBindingByStorageKey(this.bindingKey(subjectType, subjectKey));
  }

  private async getBindingByStorageKey(storageKey: string): Promise<MobilityBinding | null> {
    const raw = await this.r.get(storageKey);
    if (!raw) return null;

    try {
      return JSON.parse(raw) as MobilityBinding;
    } catch {
      return null;
    }
  }

  private async getTimelineEvents(sessionId: string): Promise<MobilityTimelineEvent[]> {
    const raw = await this.r.get(this.timelineKey(sessionId));
    if (!raw) return [];

    try {
      const parsed = JSON.parse(raw);
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter((event): event is MobilityTimelineEvent => typeof event === "object" && event !== null)
        .sort((a, b) => (Date.parse(a.happenedAt) || 0) - (Date.parse(b.happenedAt) || 0));
    } catch {
      return [];
    }
  }

  private async resolveTimelineEvents(
    sessionId: string,
    fallbackSession: LoginSession | null,
  ): Promise<MobilityTimelineEvent[]> {
    const events = await this.getTimelineEvents(sessionId);
    if (events.length > 0) return events;
    if (!fallbackSession) return [];
    return [
      this.buildTimelineLoginEvent({
        ip: fallbackSession.ip,
        ipLocation: fallbackSession.ipLocation,
        happenedAt: fallbackSession.loginTime,
      }),
    ];
  }

  private async appendTimelineEvent(
    sessionId: string,
    event: MobilityTimelineEvent,
    fallbackTtlSeconds: number | null,
    seedLoginEvent?: MobilityTimelineEvent,
  ): Promise<void> {
    const key = this.timelineKey(sessionId);
    const [events, currentTtl] = await Promise.all([this.getTimelineEvents(sessionId), this.r.ttl(key)]);
    const nextEvents = events.length === 0 && seedLoginEvent ? [seedLoginEvent, event] : [...events, event];

    if (currentTtl > 0) {
      await this.r.set(key, JSON.stringify(nextEvents), "EX", currentTtl);
      return;
    }

    if (fallbackTtlSeconds && fallbackTtlSeconds > 0) {
      await this.r.set(key, JSON.stringify(nextEvents), "EX", fallbackTtlSeconds);
      return;
    }

    await this.r.set(key, JSON.stringify(nextEvents));
  }

  private async ensureSessionIndexTTL(sessionId: string, ttlSeconds: number): Promise<void> {
    const key = this.sessionIndexKey(sessionId);
    const currentTtl = await this.r.ttl(key);
    if (currentTtl < ttlSeconds) {
      await this.r.expire(key, ttlSeconds);
    }
  }
}

export const authMobilitySessionManager = new AuthMobilitySessionManager();
