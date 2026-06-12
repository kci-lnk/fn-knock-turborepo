import { createHash } from "node:crypto";
import type Redis from "ioredis";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { scheduleSyncReverseProxyTrustedIPs } from "./reverse-proxy-trusted-ips";
import {
  DEFAULT_AUTH_CREDENTIAL_SETTINGS,
  configManager,
  redis,
  type AuthCredentialSettings,
  type LoginSession,
} from "./redis";
import { emitSessionIpDriftEvent } from "./system-events/helpers";
import { normalizeIp } from "./ip-normalize";
import { whitelistManager } from "./whitelist-manager";

type MobilitySubjectType = "proxy-session" | "fnos-token" | "trim-media-token";
type MobilityDriftSource =
  | "proxy-session"
  | "fnos-token"
  | "session-refresh"
  | "browser-session";

type MobilityBinding = {
  version: 1;
  subjectType: MobilitySubjectType;
  subjectHash: string;
  currentIp: string;
  whitelistRecordId?: string;
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
      source: MobilityDriftSource;
      fromIp: string;
      fromIpLocation?: string;
      toIp: string;
      toIpLocation?: string;
    };

type SessionActiveIpSource = MobilityDriftSource | "login";

type SessionActiveIpDetail = {
  version: 1;
  ip: string;
  firstSeenAt: number;
  lastSeenAt: number;
  source: SessionActiveIpSource;
  ipLocation?: string;
  whitelistRecordId?: string;
};

export type SessionActiveIpEntry = {
  ip: string;
  firstSeenAt: string;
  lastSeenAt: string;
  expiresAt: string;
  source: SessionActiveIpSource;
  ipLocation?: string;
  whitelistRecordId?: string;
};

export type SessionMobilitySummary = {
  hasHistory: boolean;
  driftCount: number;
  lastDriftAt: string | null;
  lastDriftSource: MobilityDriftSource | null;
};

export type SessionMobilityDetails = {
  summary: SessionMobilitySummary;
  events: MobilityTimelineEvent[];
};

export type SessionAppAttachment = {
  subjectHash: string;
  currentIp: string;
  createdAt: string;
  lastSeenAt: string;
  expiresAt: string | null;
};

export type SessionFnosAttachment = SessionAppAttachment;
export type SessionTrimMediaAttachment = SessionAppAttachment;

type MobilityAppBinding = "fnos-app" | "trim-media-app";

type RequestIdentity = {
  sessionId: string | null;
  fnosToken: string | null;
  trimMediaToken: string | null;
  appBinding: MobilityAppBinding | null;
};

type DriftRestoreResult = {
  success: boolean;
  message?: string;
  grantType?: "session_migration" | "fnos_fingerprint_session";
};

type BootstrapOwnerResolution = {
  ownerSessionId: string;
  ownerSession: LoginSession;
};

const PREFIX = "fn_knock:auth_mobility";
const MAX_TIMELINE_EVENTS = 100;
const MAX_SESSION_ACTIVE_IPS = 32;

const parseCookieValue = (
  cookieHeader: string,
  name: string,
): string | null => {
  const segments = cookieHeader.split(";");
  let lastValue: string | null = null;

  for (const segment of segments) {
    const [rawKey, ...rest] = segment.split("=");
    if (!rawKey || rest.length === 0) continue;
    if (rawKey.trim() !== name) continue;
    const raw = rest.join("=").trim().replace(/^"|"$/g, "");
    if (!raw) continue;
    try {
      lastValue = decodeURIComponent(raw);
    } catch {
      lastValue = raw;
    }
  }

  return lastValue;
};

const parseHeaderTokenValue = (value: string | null): string | null => {
  const trimmed = value?.trim();
  if (!trimmed) return null;

  const schemeMatch = trimmed.match(/^(?:bearer|token)\s+(.+)$/i);
  if (schemeMatch?.[1]) {
    const token = schemeMatch[1].trim();
    return token || null;
  }

  return trimmed;
};

const toUnixSeconds = (iso?: string): number | null => {
  if (!iso) return null;
  const ms = Date.parse(iso);
  if (!Number.isFinite(ms)) return null;
  return Math.floor(ms / 1000);
};

const nowSeconds = () => Math.floor(Date.now() / 1000);

const normalizeForwardedPathname = (rawPath: string | null): string => {
  const value = rawPath?.trim();
  if (!value) return "";

  try {
    return new URL(value, "http://localhost").pathname;
  } catch {
    const [pathname = ""] = value.split("?");
    if (!pathname) return "";
    return pathname.startsWith("/") ? pathname : `/${pathname}`;
  }
};

const normalizeUserAgent = (userAgent: string): string =>
  userAgent.trim().toLowerCase();

const isFnosAppUserAgent = (userAgent: string): boolean => {
  const normalized = normalizeUserAgent(userAgent);
  if (!normalized) return false;

  return (
    normalized.includes("com.trim.app") ||
    normalized.includes("dart:io") ||
    normalized.includes("flutter/")
  );
};

const isTrimMediaAppUserAgent = (userAgent: string): boolean =>
  normalizeUserAgent(userAgent).includes("com.trim.media");

const isFNAppForwardedPath = (pathname: string): boolean =>
  pathname === "/trimcon" || pathname === "/websocket";

const hasFNAppRelayCookie = (cookieHeader: string): boolean =>
  cookieHeader.toLowerCase().includes("mode=relay");

const resolveAppBinding = (args: {
  userAgent: string;
  forwardedPathname: string;
  cookieHeader: string;
  fnosToken: string | null;
}): MobilityAppBinding | null => {
  if (isTrimMediaAppUserAgent(args.userAgent)) {
    return "trim-media-app";
  }

  const isFnosAppRequest =
    isFNAppForwardedPath(args.forwardedPathname) &&
    (isFnosAppUserAgent(args.userAgent) ||
      hasFNAppRelayCookie(args.cookieHeader) ||
      !!args.fnosToken);

  return isFnosAppRequest ? "fnos-app" : null;
};

export class AuthMobilitySessionManager {
  private readonly r: Redis;

  constructor() {
    this.r = redis;
  }

  inspectRequest(request: Request): RequestIdentity {
    const cookieHeader = request.headers.get("cookie") || "";
    const sessionId = parseCookieValue(
      cookieHeader,
      "x-go-reauth-proxy-session-id",
    );
    const fnosToken = parseCookieValue(cookieHeader, "fnos-token");
    const forwardedPathname = normalizeForwardedPathname(
      request.headers.get("x-forwarded-path"),
    );
    const appBinding = resolveAppBinding({
      userAgent: request.headers.get("user-agent") || "",
      forwardedPathname,
      cookieHeader,
      fnosToken,
    });
    const trimMediaToken =
      appBinding === "trim-media-app"
        ? parseHeaderTokenValue(request.headers.get("authorization")) ||
          parseHeaderTokenValue(request.headers.get("accesstoken")) ||
          parseHeaderTokenValue(request.headers.get("access-token"))
        : null;

    return {
      sessionId,
      fnosToken,
      trimMediaToken,
      appBinding,
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
    const loginEvent = this.buildTimelineLoginEvent({
      ip: args.ip,
      ipLocation: args.ipLocation,
    });
    pipeline.set(
      this.bindingKey("proxy-session", args.sessionId),
      JSON.stringify(binding),
      "EX",
      ttlSeconds,
    );
    pipeline.set(
      this.timelineKey(args.sessionId),
      JSON.stringify([loginEvent] satisfies MobilityTimelineEvent[]),
      "EX",
      ttlSeconds,
    );
    pipeline.set(
      this.summaryKey(args.sessionId),
      JSON.stringify(this.buildMobilitySummary([loginEvent])),
      "EX",
      ttlSeconds,
    );
    pipeline.sadd(
      this.sessionIndexKey(args.sessionId),
      this.bindingKey("proxy-session", args.sessionId),
    );
    pipeline.expire(this.sessionIndexKey(args.sessionId), ttlSeconds);
    pipeline.set(
      this.whitelistOwnerKey(args.whitelistRecordId),
      args.sessionId,
      "EX",
      ttlSeconds,
    );
    await pipeline.exec();

    await this.recordSessionActiveIp({
      sessionId: args.sessionId,
      clientIp: args.ip,
      source: "login",
      ...(args.ipLocation ? { ipLocation: args.ipLocation } : {}),
      whitelistRecordId: args.whitelistRecordId,
      syncReason: "mobility-login-session",
    });
  }

  async recordBrowserSessionLogin(args: {
    sessionId: string;
    ip: string;
    ipLocation?: string;
  }): Promise<void> {
    await this.recordSessionActiveIp({
      sessionId: args.sessionId,
      clientIp: args.ip,
      source: "login",
      ...(args.ipLocation ? { ipLocation: args.ipLocation } : {}),
      syncReason: "browser-session-login",
    });
  }

  async syncTrustedRequest(request: Request, clientIp: string): Promise<void> {
    const identity = this.inspectRequest(request);

    if (identity.sessionId) {
      await this.refreshProxySessionBinding(identity.sessionId, clientIp);
    }

    if (identity.fnosToken) {
      await this.refreshFnosBinding(
        identity.fnosToken,
        clientIp,
        identity.sessionId,
      );
    }

    if (identity.trimMediaToken) {
      await this.refreshTrimMediaBinding(
        identity.trimMediaToken,
        clientIp,
        identity.sessionId,
      );
    }
  }

  async tryRestoreAccess(
    request: Request,
    clientIp: string,
  ): Promise<DriftRestoreResult> {
    const identity = this.inspectRequest(request);

    if (identity.fnosToken) {
      const restored = await this.restoreFnosToken(
        identity.fnosToken,
        clientIp,
      );
      if (restored) {
        return {
          success: true,
          message: "Authorized by fnos fingerprint session",
          grantType: "fnos_fingerprint_session",
        };
      }
    }

    if (identity.trimMediaToken) {
      const restored = await this.restoreTrimMediaToken(
        identity.trimMediaToken,
        clientIp,
      );
      if (restored) {
        return {
          success: true,
          message: "Authorized by trim media token binding",
          grantType: "fnos_fingerprint_session",
        };
      }
    }

    if (identity.appBinding === "fnos-app") {
      const restored = await this.restoreAnonymousFnosApp(clientIp);
      if (restored) {
        return {
          success: true,
          message: "Authorized by fnos app bootstrap session",
          grantType: "fnos_fingerprint_session",
        };
      }
    }

    if (identity.appBinding === "trim-media-app") {
      const restored = await this.restoreTrimMediaApp(clientIp);
      if (restored) {
        return {
          success: true,
          message: "Authorized by trim media app binding",
          grantType: "fnos_fingerprint_session",
        };
      }
    }

    if (identity.sessionId) {
      const restored = await this.restoreProxySession(
        identity.sessionId,
        clientIp,
      );
      if (restored) {
        return {
          success: true,
          message: "Authorized by session IP migration",
          grantType: "session_migration",
        };
      }
    }

    return { success: false };
  }

  async hasResolvableMobilityAccess(
    request: Request,
    clientIp: string,
  ): Promise<boolean> {
    const identity = this.inspectRequest(request);
    if (!identity.fnosToken && !identity.trimMediaToken && !identity.appBinding)
      return false;

    if (identity.fnosToken) {
      const binding = await this.getBinding("fnos-token", identity.fnosToken);
      if (binding?.ownerSessionId) {
        const owner = await this.resolveSessionOwner(binding.ownerSessionId);
        if (owner) {
          return !!this.resolveFnosSessionTTL(owner.ownerSession.expiresAt);
        }
      }
    }

    if (identity.trimMediaToken) {
      const binding = await this.getBinding(
        "trim-media-token",
        identity.trimMediaToken,
      );
      if (binding?.ownerSessionId) {
        const owner = await this.resolveSessionOwner(binding.ownerSessionId);
        if (owner) {
          return !!this.resolveFnosSessionTTL(owner.ownerSession.expiresAt);
        }
      }
    }

    if (identity.appBinding === "trim-media-app") {
      return this.hasActiveSessionAtIp(clientIp);
    }

    if (identity.appBinding === "fnos-app") {
      return !!(await this.resolveBootstrapOwner(clientIp));
    }

    return false;
  }

  async destroySession(sessionId: string): Promise<void> {
    const sessionKey = this.sessionIndexKey(sessionId);
    const subjectKeys = await this.r.smembers(sessionKey);
    const uniqueWhitelistRecordIds = new Set<string>();
    const proxyBinding = await this.getBinding("proxy-session", sessionId);
    const activeIpDetails = await this.getAllSessionActiveIpDetails(sessionId);

    if (proxyBinding?.whitelistRecordId) {
      uniqueWhitelistRecordIds.add(proxyBinding.whitelistRecordId);
    }

    for (const subjectKey of subjectKeys) {
      const binding = await this.getBindingByStorageKey(subjectKey);
      if (binding?.whitelistRecordId) {
        uniqueWhitelistRecordIds.add(binding.whitelistRecordId);
      }
    }
    for (const detail of activeIpDetails) {
      if (detail.whitelistRecordId) {
        uniqueWhitelistRecordIds.add(detail.whitelistRecordId);
      }
    }

    const pipeline = this.r.pipeline();
    pipeline.del(this.bindingKey("proxy-session", sessionId));
    pipeline.del(this.timelineKey(sessionId));
    pipeline.del(this.summaryKey(sessionId));
    pipeline.del(this.activeIpZsetKey(sessionId));
    pipeline.del(this.activeIpDetailsKey(sessionId));
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

  async getSessionWhitelistRecordId(sessionId: string): Promise<string | null> {
    const binding = await this.getBinding("proxy-session", sessionId);
    return binding?.whitelistRecordId ?? null;
  }

  async listSessionWhitelistRecordIds(sessionId: string): Promise<string[]> {
    const recordIds = new Set<string>();
    const binding = await this.getBinding("proxy-session", sessionId);
    if (binding?.whitelistRecordId) {
      recordIds.add(binding.whitelistRecordId);
    }
    for (const detail of await this.getAllSessionActiveIpDetails(sessionId)) {
      if (detail.whitelistRecordId) {
        recordIds.add(detail.whitelistRecordId);
      }
    }
    return [...recordIds];
  }

  async syncSessionIp(args: {
    sessionId: string;
    clientIp: string;
    source: MobilityDriftSource;
    ipLocation?: string;
    sessionPatch?: Partial<LoginSession>;
    syncReason: string;
  }): Promise<LoginSession | null> {
    const session = await configManager.getSession(args.sessionId);
    if (!session) return null;

    const previousIp = session.ip;
    const previousIpLocation = session.ipLocation;
    const settings = await configManager.getAuthCredentialSettings();
    const normalizedPreviousIp =
      normalizeIp(previousIp) || String(previousIp || "").trim();
    const normalizedClientIp =
      normalizeIp(args.clientIp) || String(args.clientIp || "").trim();
    const ipChanged =
      normalizedPreviousIp && normalizedClientIp
        ? normalizedPreviousIp !== normalizedClientIp
        : previousIp !== args.clientIp;
    const shouldEmitIpDriftEvent =
      ipChanged && settings.session_ip_mobility_enabled !== true;
    const nextSessionPatch: Partial<LoginSession> = {
      ...args.sessionPatch,
      ip: args.clientIp,
    };

    if (ipChanged) {
      nextSessionPatch.ipLocation =
        args.ipLocation && args.ipLocation.trim().length > 0
          ? args.ipLocation
          : undefined;
    } else if (typeof args.ipLocation === "string") {
      nextSessionPatch.ipLocation =
        args.ipLocation.trim().length > 0 ? args.ipLocation : undefined;
    }

    const nextSession = await configManager.updateSession(
      args.sessionId,
      nextSessionPatch,
    );
    if (!nextSession) return null;

    if (shouldEmitIpDriftEvent) {
      await this.appendTimelineEvent(
        args.sessionId,
        this.buildTimelineDriftEvent({
          source: args.source,
          fromIp: previousIp,
          fromIpLocation: previousIpLocation,
          toIp: args.clientIp,
          toIpLocation: args.ipLocation,
        }),
        this.resolveProxySessionTTL(toUnixSeconds(nextSession.expiresAt)),
        this.buildTimelineLoginEvent({
          ip: previousIp,
          ipLocation: previousIpLocation,
          happenedAt: session.loginTime,
        }),
      );
      await emitSessionIpDriftEvent({
        sessionId: args.sessionId,
        authMethod: session.method,
        credentialId: session.credentialId,
        credentialName: session.credentialName,
        ...(session.linkedTotpName
          ? { linkedTotpName: session.linkedTotpName }
          : {}),
        ...(session.comment ? { sessionComment: session.comment } : {}),
        driftSource: args.source,
        fromIp: previousIp,
        ...(previousIpLocation ? { fromIpLocation: previousIpLocation } : {}),
        toIp: args.clientIp,
        ...(args.ipLocation ? { toIpLocation: args.ipLocation } : {}),
        loginTime: session.loginTime,
      });
      scheduleSyncReverseProxyTrustedIPs({
        reason: args.syncReason,
      });
    }

    await this.recordSessionActiveIp({
      sessionId: args.sessionId,
      session: nextSession,
      clientIp: args.clientIp,
      source: args.source,
      ...(args.ipLocation ? { ipLocation: args.ipLocation } : {}),
      settings,
      syncReason: args.syncReason,
    });

    await ipLocationService.registerUsage(args.clientIp, [
      ipLocationRefs.session(args.sessionId),
      ipLocationRefs.sessionTimeline(args.sessionId),
    ]);

    return nextSession;
  }

  private async refreshProxySessionBinding(
    sessionId: string,
    clientIp: string,
  ): Promise<void> {
    const session = await configManager.getSession(sessionId);
    if (!session) return;

    const existing = await this.getBinding("proxy-session", sessionId);
    if (!existing) {
      if (await this.isSessionIpMobilityEnabled()) {
        const nextIpLocation = clientIp
          ? await ipLocationService.getCachedLocation(clientIp)
          : "";
        await this.syncSessionIp({
          sessionId,
          clientIp,
          source: "session-refresh",
          ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
          syncReason: "mobility-session-refresh",
        });
      }
      return;
    }

    existing.currentIp = clientIp;
    existing.lastSeenAt = new Date().toISOString();
    existing.expireAt = toUnixSeconds(session.expiresAt);
    await this.r.set(
      this.bindingKey("proxy-session", sessionId),
      JSON.stringify(existing),
      "KEEPTTL",
    );
    if (session.ip !== clientIp) {
      const nextIpLocation = clientIp
        ? await ipLocationService.getCachedLocation(clientIp)
        : "";
      await this.syncSessionIp({
        sessionId,
        clientIp,
        source: "session-refresh",
        ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
        syncReason: "mobility-session-refresh",
      });
    }

    await this.recordSessionActiveIp({
      sessionId,
      session,
      clientIp,
      source: "session-refresh",
      syncReason: "mobility-session-refresh",
    });
  }

  async getSessionMobilitySummary(
    sessionId: string,
  ): Promise<SessionMobilitySummary> {
    const session = await configManager.getSession(sessionId);
    const [events, storedSummary] = await Promise.all([
      this.resolveTimelineEvents(sessionId, session),
      this.getStoredSummary(sessionId),
    ]);
    return storedSummary ?? this.buildMobilitySummary(events);
  }

  async getSessionMobilityDetails(
    sessionId: string,
  ): Promise<SessionMobilityDetails> {
    const session = await configManager.getSession(sessionId);
    const [events, storedSummary] = await Promise.all([
      this.resolveTimelineEvents(sessionId, session),
      this.getStoredSummary(sessionId),
    ]);
    return {
      summary: storedSummary ?? this.buildMobilitySummary(events),
      events,
    };
  }

  async listSessionFnosAttachments(
    sessionId: string,
  ): Promise<SessionFnosAttachment[]> {
    return this.listSessionAttachments(sessionId, "fnos-token");
  }

  async listSessionTrimMediaAttachments(
    sessionId: string,
  ): Promise<SessionTrimMediaAttachment[]> {
    return this.listSessionAttachments(sessionId, "trim-media-token");
  }

  private async listSessionAttachments(
    sessionId: string,
    subjectType: "fnos-token" | "trim-media-token",
  ): Promise<SessionAppAttachment[]> {
    const sessionKey = this.sessionIndexKey(sessionId);
    const subjectKeys = await this.r.smembers(sessionKey);
    const attachmentKeys = subjectKeys.filter((key) =>
      key.startsWith(`${PREFIX}:binding:${subjectType}:`),
    );
    if (attachmentKeys.length === 0) {
      return [];
    }

    const resolved = await Promise.all(
      attachmentKeys.map(async (storageKey) => {
        const binding = await this.getBindingByStorageKey(storageKey);
        return { storageKey, binding };
      }),
    );

    const staleKeys = resolved
      .filter(
        ({ binding }) =>
          !binding ||
          binding.subjectType !== subjectType ||
          binding.ownerSessionId !== sessionId,
      )
      .map(({ storageKey }) => storageKey);

    if (staleKeys.length > 0) {
      await this.r.srem(sessionKey, ...staleKeys);
    }

    return resolved
      .flatMap(({ binding }) => {
        if (
          !binding ||
          binding.subjectType !== subjectType ||
          binding.ownerSessionId !== sessionId
        ) {
          return [];
        }

        return [
          {
            subjectHash: binding.subjectHash,
            currentIp: binding.currentIp,
            createdAt: binding.createdAt,
            lastSeenAt: binding.lastSeenAt,
            expiresAt: binding.expireAt
              ? new Date(binding.expireAt * 1000).toISOString()
              : null,
          } satisfies SessionAppAttachment,
        ];
      })
      .sort((a, b) => {
        return (
          (Date.parse(b.lastSeenAt) || 0) - (Date.parse(a.lastSeenAt) || 0)
        );
      });
  }

  private async refreshFnosBinding(
    fnosToken: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    const storageKey = this.bindingKey("fnos-token", fnosToken);
    let existing = await this.getBinding("fnos-token", fnosToken);
    if (!sessionId) {
      if (existing?.ownerSessionId) {
        const owner = await this.resolveSessionOwner(existing.ownerSessionId);
        if (!owner) {
          const orphanedBinding: MobilityBinding = {
            ...existing,
            ownerSessionId: undefined,
            lastSeenAt: new Date().toISOString(),
          };
          const pipeline = this.r.pipeline();
          pipeline.set(storageKey, JSON.stringify(orphanedBinding), "KEEPTTL");
          pipeline.srem(
            this.sessionIndexKey(existing.ownerSessionId),
            storageKey,
          );
          await pipeline.exec();
          existing = orphanedBinding;
        } else {
          const ttlSeconds = this.resolveFnosSessionTTL(
            owner.ownerSession.expiresAt,
          );
          if (!ttlSeconds) return;

          existing.currentIp = clientIp;
          existing.expireAt = toUnixSeconds(owner.ownerSession.expiresAt);
          existing.lastSeenAt = new Date().toISOString();
          await this.r.set(
            storageKey,
            JSON.stringify(existing),
            "EX",
            ttlSeconds,
          );
          await this.r.sadd(
            this.sessionIndexKey(owner.ownerSessionId),
            storageKey,
          );
          await this.ensureSessionIndexTTL(
            owner.ownerSessionId,
            this.resolveProxySessionTTL(
              toUnixSeconds(owner.ownerSession.expiresAt),
            ) || ttlSeconds,
          );
          return;
        }
      }

      const bootstrap = await this.resolveBootstrapOwner(clientIp);
      if (!bootstrap) return;

      const { ownerSessionId, ownerSession } = bootstrap;

      const sessionTtl = this.resolveProxySessionTTL(
        toUnixSeconds(ownerSession.expiresAt),
      );
      const fnosTtl = this.resolveFnosSessionTTL(ownerSession.expiresAt);
      if (!sessionTtl || !fnosTtl) return;

      const binding: MobilityBinding = existing
        ? {
            ...existing,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : this.buildBinding({
            subjectType: "fnos-token",
            subjectKey: fnosToken,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
          });

      await this.r.set(
        this.bindingKey("fnos-token", fnosToken),
        JSON.stringify(binding),
        "EX",
        fnosTtl,
      );
      await this.r.sadd(
        this.sessionIndexKey(ownerSessionId),
        this.bindingKey("fnos-token", fnosToken),
      );
      await this.ensureSessionIndexTTL(ownerSessionId, sessionTtl);
      return;
    }

    const session = await configManager.getSession(sessionId);
    if (!session) return;

    if (existing?.ownerSessionId && existing.ownerSessionId !== sessionId) {
      const existingOwner = await configManager.getSession(
        existing.ownerSessionId,
      );
      if (existingOwner) return;
      await this.r.srem(
        this.sessionIndexKey(existing.ownerSessionId),
        storageKey,
      );
    }

    const ttlSeconds = this.resolveFnosSessionTTL(session.expiresAt);
    if (!ttlSeconds) return;

    const binding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
          lastSeenAt: new Date().toISOString(),
        }
      : this.buildBinding({
          subjectType: "fnos-token",
          subjectKey: fnosToken,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
        });

    await this.r.set(storageKey, JSON.stringify(binding), "EX", ttlSeconds);
    await this.r.sadd(this.sessionIndexKey(sessionId), storageKey);
    const sessionTtl = this.resolveProxySessionTTL(
      toUnixSeconds(session.expiresAt),
    );
    if (sessionTtl) {
      await this.ensureSessionIndexTTL(sessionId, sessionTtl);
    }
  }

  private async refreshTrimMediaBinding(
    trimMediaToken: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    const storageKey = this.bindingKey("trim-media-token", trimMediaToken);
    let existing = await this.getBinding("trim-media-token", trimMediaToken);
    if (!sessionId) {
      if (existing?.ownerSessionId) {
        const owner = await this.resolveSessionOwner(existing.ownerSessionId);
        if (!owner) {
          const orphanedBinding: MobilityBinding = {
            ...existing,
            ownerSessionId: undefined,
            lastSeenAt: new Date().toISOString(),
          };
          const pipeline = this.r.pipeline();
          pipeline.set(storageKey, JSON.stringify(orphanedBinding), "KEEPTTL");
          pipeline.srem(
            this.sessionIndexKey(existing.ownerSessionId),
            storageKey,
          );
          await pipeline.exec();
          existing = orphanedBinding;
        } else {
          const ttlSeconds = this.resolveFnosSessionTTL(
            owner.ownerSession.expiresAt,
          );
          if (!ttlSeconds) return;

          existing.currentIp = clientIp;
          existing.expireAt = toUnixSeconds(owner.ownerSession.expiresAt);
          existing.lastSeenAt = new Date().toISOString();
          await this.r.set(
            storageKey,
            JSON.stringify(existing),
            "EX",
            ttlSeconds,
          );
          await this.r.sadd(
            this.sessionIndexKey(owner.ownerSessionId),
            storageKey,
          );
          await this.ensureSessionIndexTTL(
            owner.ownerSessionId,
            this.resolveProxySessionTTL(
              toUnixSeconds(owner.ownerSession.expiresAt),
            ) || ttlSeconds,
          );
          return;
        }
      }

      const bootstrap = await this.resolveBootstrapOwner(clientIp);
      if (!bootstrap) return;

      const { ownerSessionId, ownerSession } = bootstrap;

      const sessionTtl = this.resolveProxySessionTTL(
        toUnixSeconds(ownerSession.expiresAt),
      );
      const trimMediaTtl = this.resolveFnosSessionTTL(ownerSession.expiresAt);
      if (!sessionTtl || !trimMediaTtl) return;

      const binding: MobilityBinding = existing
        ? {
            ...existing,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : this.buildBinding({
            subjectType: "trim-media-token",
            subjectKey: trimMediaToken,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
          });

      await this.r.set(storageKey, JSON.stringify(binding), "EX", trimMediaTtl);
      await this.r.sadd(this.sessionIndexKey(ownerSessionId), storageKey);
      await this.ensureSessionIndexTTL(ownerSessionId, sessionTtl);
      return;
    }

    const session = await configManager.getSession(sessionId);
    if (!session) return;

    if (existing?.ownerSessionId && existing.ownerSessionId !== sessionId) {
      const existingOwner = await configManager.getSession(
        existing.ownerSessionId,
      );
      if (existingOwner) return;
      await this.r.srem(
        this.sessionIndexKey(existing.ownerSessionId),
        storageKey,
      );
    }

    const ttlSeconds = this.resolveFnosSessionTTL(session.expiresAt);
    if (!ttlSeconds) return;

    const binding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
          lastSeenAt: new Date().toISOString(),
        }
      : this.buildBinding({
          subjectType: "trim-media-token",
          subjectKey: trimMediaToken,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
        });

    await this.r.set(storageKey, JSON.stringify(binding), "EX", ttlSeconds);
    await this.r.sadd(this.sessionIndexKey(sessionId), storageKey);
    const sessionTtl = this.resolveProxySessionTTL(
      toUnixSeconds(session.expiresAt),
    );
    if (sessionTtl) {
      await this.ensureSessionIndexTTL(sessionId, sessionTtl);
    }
  }

  private async listActiveSessionsByIp(
    clientIp: string,
  ): Promise<BootstrapOwnerResolution[]> {
    const normalizedIp = normalizeIp(clientIp) || String(clientIp || "").trim();
    if (!normalizedIp) return [];

    const settings = await configManager.getAuthCredentialSettings();
    const sessions = await configManager.listSessions();
    if (!settings.session_ip_mobility_enabled) {
      return sessions
        .filter((session) => session.data.ip === normalizedIp)
        .map((session) => ({
          ownerSessionId: session.id,
          ownerSession: session.data,
        }));
    }

    const resolved = await Promise.all(
      sessions.map(async (session) => {
        const active = await this.isSessionActiveAtIp(
          session.id,
          session.data,
          normalizedIp,
          settings,
        );
        if (!active) return null;
        return {
          ownerSessionId: session.id,
          ownerSession: session.data,
        } satisfies BootstrapOwnerResolution;
      }),
    );
    return resolved.filter(
      (entry): entry is BootstrapOwnerResolution => entry !== null,
    );
  }

  private async hasActiveSessionAtIp(clientIp: string): Promise<boolean> {
    const sessions = await this.listActiveSessionsByIp(clientIp);
    return sessions.length > 0;
  }

  private async resolveBootstrapOwner(
    clientIp: string,
  ): Promise<BootstrapOwnerResolution | null> {
    const candidateSessions = await this.listActiveSessionsByIp(clientIp);
    if (candidateSessions.length !== 1) return null;

    const [candidate] = candidateSessions;
    if (!candidate) return null;

    return candidate;
  }

  private async resolveSessionOwner(
    ownerSessionId: string,
  ): Promise<BootstrapOwnerResolution | null> {
    const ownerSession = await configManager.getSession(ownerSessionId);
    if (!ownerSession) return null;

    return {
      ownerSessionId,
      ownerSession,
    };
  }

  private async restoreFnosToken(
    fnosToken: string,
    clientIp: string,
  ): Promise<boolean> {
    let binding = await this.getBinding("fnos-token", fnosToken);
    if (binding?.ownerSessionId) {
      const owner = await this.resolveSessionOwner(binding.ownerSessionId);
      if (!owner) {
        const orphanedBinding: MobilityBinding = {
          ...binding,
          ownerSessionId: undefined,
          lastSeenAt: new Date().toISOString(),
        };
        await this.r.srem(
          this.sessionIndexKey(binding.ownerSessionId),
          this.bindingKey("fnos-token", fnosToken),
        );
        await this.r.set(
          this.bindingKey("fnos-token", fnosToken),
          JSON.stringify(orphanedBinding),
          "KEEPTTL",
        );
        binding = orphanedBinding;
      }
    }

    if (!binding?.ownerSessionId) {
      const bootstrap = await this.resolveBootstrapOwner(clientIp);
      if (!bootstrap) return false;

      const ttlSeconds = this.resolveFnosSessionTTL(
        bootstrap.ownerSession.expiresAt,
      );
      if (!ttlSeconds) return false;

      binding = binding
        ? {
            ...binding,
            currentIp: clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : this.buildBinding({
            subjectType: "fnos-token",
            subjectKey: fnosToken,
            currentIp: clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
          });

      await this.r.set(
        this.bindingKey("fnos-token", fnosToken),
        JSON.stringify(binding),
        "EX",
        ttlSeconds,
      );
      await this.r.sadd(
        this.sessionIndexKey(bootstrap.ownerSessionId),
        this.bindingKey("fnos-token", fnosToken),
      );
      const sessionTtl = this.resolveProxySessionTTL(
        toUnixSeconds(bootstrap.ownerSession.expiresAt),
      );
      if (sessionTtl) {
        await this.ensureSessionIndexTTL(bootstrap.ownerSessionId, sessionTtl);
      }
    }

    const ownerSessionId = binding.ownerSessionId;
    if (!ownerSessionId) return false;

    const owner = await this.resolveSessionOwner(ownerSessionId);
    if (!owner) return false;
    const ownerSession = owner.ownerSession;

    const ttlSeconds = this.resolveFnosSessionTTL(ownerSession.expiresAt);
    if (!ttlSeconds) return false;

    const nextIpLocation = clientIp
      ? await ipLocationService.getCachedLocation(clientIp)
      : "";
    binding.currentIp = clientIp;
    binding.expireAt = toUnixSeconds(ownerSession.expiresAt);
    binding.lastSeenAt = new Date().toISOString();
    await this.r.set(
      this.bindingKey("fnos-token", fnosToken),
      JSON.stringify(binding),
      "EX",
      ttlSeconds,
    );

    const updatedSession = await this.syncSessionIp({
      sessionId: ownerSessionId,
      clientIp,
      source: "fnos-token",
      ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
      syncReason: "fnos-token-restore",
    });
    const sessionTtl = this.resolveProxySessionTTL(
      toUnixSeconds(updatedSession?.expiresAt),
    );
    if (updatedSession && sessionTtl) {
      await this.ensureSessionIndexTTL(ownerSessionId, sessionTtl);
      await this.r.sadd(
        this.sessionIndexKey(ownerSessionId),
        this.bindingKey("fnos-token", fnosToken),
      );
    }

    return true;
  }

  private async restoreAnonymousFnosApp(clientIp: string): Promise<boolean> {
    const bootstrap = await this.resolveBootstrapOwner(clientIp);
    if (!bootstrap) return false;

    await ipLocationService.registerUsage(clientIp, [
      ipLocationRefs.session(bootstrap.ownerSessionId),
      ipLocationRefs.sessionTimeline(bootstrap.ownerSessionId),
    ]);

    return true;
  }

  private async restoreTrimMediaApp(clientIp: string): Promise<boolean> {
    const sessions = await this.listActiveSessionsByIp(clientIp);
    if (sessions.length === 0) return false;

    const usageRefs = [
      ...new Set(
        sessions.flatMap((session) => [
          ipLocationRefs.session(session.ownerSessionId),
          ipLocationRefs.sessionTimeline(session.ownerSessionId),
        ]),
      ),
    ];

    await ipLocationService.registerUsage(clientIp, usageRefs);

    return true;
  }

  private async restoreTrimMediaToken(
    trimMediaToken: string,
    clientIp: string,
  ): Promise<boolean> {
    let binding = await this.getBinding("trim-media-token", trimMediaToken);
    if (binding?.ownerSessionId) {
      const owner = await this.resolveSessionOwner(binding.ownerSessionId);
      if (!owner) {
        const orphanedBinding: MobilityBinding = {
          ...binding,
          ownerSessionId: undefined,
          lastSeenAt: new Date().toISOString(),
        };
        await this.r.srem(
          this.sessionIndexKey(binding.ownerSessionId),
          this.bindingKey("trim-media-token", trimMediaToken),
        );
        await this.r.set(
          this.bindingKey("trim-media-token", trimMediaToken),
          JSON.stringify(orphanedBinding),
          "KEEPTTL",
        );
        binding = orphanedBinding;
      }
    }

    if (!binding?.ownerSessionId) {
      const bootstrap = await this.resolveBootstrapOwner(clientIp);
      if (!bootstrap) return false;

      const ttlSeconds = this.resolveFnosSessionTTL(
        bootstrap.ownerSession.expiresAt,
      );
      if (!ttlSeconds) return false;

      binding = binding
        ? {
            ...binding,
            currentIp: clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : this.buildBinding({
            subjectType: "trim-media-token",
            subjectKey: trimMediaToken,
            currentIp: clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
          });

      await this.r.set(
        this.bindingKey("trim-media-token", trimMediaToken),
        JSON.stringify(binding),
        "EX",
        ttlSeconds,
      );
      await this.r.sadd(
        this.sessionIndexKey(bootstrap.ownerSessionId),
        this.bindingKey("trim-media-token", trimMediaToken),
      );
      const sessionTtl = this.resolveProxySessionTTL(
        toUnixSeconds(bootstrap.ownerSession.expiresAt),
      );
      if (sessionTtl) {
        await this.ensureSessionIndexTTL(bootstrap.ownerSessionId, sessionTtl);
      }
    }

    const ownerSessionId = binding.ownerSessionId;
    if (!ownerSessionId) return false;

    const owner = await this.resolveSessionOwner(ownerSessionId);
    if (!owner) return false;
    const ownerSession = owner.ownerSession;

    const ttlSeconds = this.resolveFnosSessionTTL(ownerSession.expiresAt);
    if (!ttlSeconds) return false;

    const nextIpLocation = clientIp
      ? await ipLocationService.getCachedLocation(clientIp)
      : "";
    binding.currentIp = clientIp;
    binding.expireAt = toUnixSeconds(ownerSession.expiresAt);
    binding.lastSeenAt = new Date().toISOString();
    await this.r.set(
      this.bindingKey("trim-media-token", trimMediaToken),
      JSON.stringify(binding),
      "EX",
      ttlSeconds,
    );

    const updatedSession = await this.syncSessionIp({
      sessionId: ownerSessionId,
      clientIp,
      // Reuse the existing FNOS fingerprint drift bucket for UI and events.
      source: "fnos-token",
      ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
      syncReason: "trim-media-token-restore",
    });
    const sessionTtl = this.resolveProxySessionTTL(
      toUnixSeconds(updatedSession?.expiresAt),
    );
    if (updatedSession && sessionTtl) {
      await this.ensureSessionIndexTTL(ownerSessionId, sessionTtl);
      await this.r.sadd(
        this.sessionIndexKey(ownerSessionId),
        this.bindingKey("trim-media-token", trimMediaToken),
      );
    }

    return true;
  }

  private async restoreProxySession(
    sessionId: string,
    clientIp: string,
  ): Promise<boolean> {
    const session = await configManager.getSession(sessionId);
    if (!session) return false;

    let binding = await this.getBinding("proxy-session", sessionId);
    const mobilityEnabled = await this.isSessionIpMobilityEnabled();
    if (mobilityEnabled) {
      if (session.ip === clientIp) {
        return false;
      }

      const nextIpLocation = clientIp
        ? await ipLocationService.getCachedLocation(clientIp)
        : "";
      if (binding) {
        binding.currentIp = clientIp;
        binding.expireAt = toUnixSeconds(session.expiresAt);
        binding.lastSeenAt = new Date().toISOString();
        await this.r.set(
          this.bindingKey("proxy-session", sessionId),
          JSON.stringify(binding),
          "KEEPTTL",
        );
      }

      await this.syncSessionIp({
        sessionId,
        clientIp,
        source: "proxy-session",
        ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
        syncReason: "proxy-session-restore",
      });

      return true;
    }

    if (!binding) {
      return false;
    }

    if (!binding.whitelistRecordId) {
      return false;
    }

    const movedRecord = await whitelistManager.moveRecordToIP(
      binding.whitelistRecordId,
      clientIp,
    );
    if (!movedRecord) return false;

    binding.currentIp = clientIp;
    binding.expireAt = movedRecord.expireAt ?? toUnixSeconds(session.expiresAt);
    binding.lastSeenAt = new Date().toISOString();
    await this.r.set(
      this.bindingKey("proxy-session", sessionId),
      JSON.stringify(binding),
      "KEEPTTL",
    );

    await this.syncSessionIp({
      sessionId,
      clientIp,
      source: "proxy-session",
      ...(movedRecord.ipLocation ? { ipLocation: movedRecord.ipLocation } : {}),
      syncReason: "proxy-session-restore",
    });

    return true;
  }

  async isSessionIpMobilityEnabled(): Promise<boolean> {
    const settings = await configManager.getAuthCredentialSettings();
    return settings.session_ip_mobility_enabled === true;
  }

  async listEffectiveSessionIps(
    sessionId: string,
    session: LoginSession,
  ): Promise<string[]> {
    const settings = await configManager.getAuthCredentialSettings();
    if (!settings.session_ip_mobility_enabled) {
      const currentIp =
        normalizeIp(session.ip) || String(session.ip || "").trim();
      return currentIp ? [currentIp] : [];
    }

    const entries = await this.listSessionActiveIpDetails(
      sessionId,
      session,
      settings,
    );
    const ips = new Set(entries.map((entry) => entry.ip).filter(Boolean));
    return [...ips].sort((left, right) => left.localeCompare(right));
  }

  async listSessionActiveIpEntries(
    sessionId: string,
    session?: LoginSession | null,
  ): Promise<SessionActiveIpEntry[]> {
    const settings = await configManager.getAuthCredentialSettings();
    if (!settings.session_ip_mobility_enabled) {
      return [];
    }

    const resolvedSession =
      session ?? (await configManager.getSession(sessionId));
    const details = await this.listSessionActiveIpDetails(
      sessionId,
      resolvedSession,
      settings,
    );
    const windowSeconds = this.getSessionIpMobilityWindowSeconds(settings);
    const sessionExpireAt = toUnixSeconds(resolvedSession?.expiresAt);

    return details.map((detail) => {
      const expiresAt = Math.min(
        sessionExpireAt ?? detail.lastSeenAt + windowSeconds,
        detail.lastSeenAt + windowSeconds,
      );
      return {
        ip: detail.ip,
        firstSeenAt: new Date(detail.firstSeenAt * 1000).toISOString(),
        lastSeenAt: new Date(detail.lastSeenAt * 1000).toISOString(),
        expiresAt: new Date(expiresAt * 1000).toISOString(),
        source: detail.source,
        ...(detail.ipLocation ? { ipLocation: detail.ipLocation } : {}),
        ...(detail.whitelistRecordId
          ? { whitelistRecordId: detail.whitelistRecordId }
          : {}),
      };
    });
  }

  async reconcileSessionIpMobilityPolicy(
    previous: AuthCredentialSettings,
    next: AuthCredentialSettings,
    options: { scheduleSync?: boolean } = {},
  ): Promise<void> {
    const shouldScheduleSync = options.scheduleSync !== false;
    const sessions = await configManager.listSessions();
    if (!next.session_ip_mobility_enabled) {
      for (const session of sessions) {
        await this.cleanupSessionActiveIpState(session.id, session.data, {
          preserveLegacySingleSlot: true,
        });
      }
      if (shouldScheduleSync) {
        scheduleSyncReverseProxyTrustedIPs({
          reason: "session-ip-mobility-disabled",
          delayMs: 0,
        });
      }
      return;
    }

    const shouldSeedCurrentIp =
      previous.session_ip_mobility_enabled !== true &&
      next.session_ip_mobility_enabled === true;

    for (const session of sessions) {
      const currentIp =
        normalizeIp(session.data.ip) || String(session.data.ip || "").trim();
      if (shouldSeedCurrentIp && currentIp) {
        await this.recordSessionActiveIp({
          sessionId: session.id,
          session: session.data,
          clientIp: currentIp,
          source: "session-refresh",
          ...(session.data.ipLocation
            ? { ipLocation: session.data.ipLocation }
            : {}),
          whitelistRecordId: session.data.postLoginIpGrantRecordId || undefined,
          settings: next,
          syncReason: "session-ip-mobility-reconcile",
          scheduleSync: shouldScheduleSync,
        });
      }
      await this.pruneSessionActiveIps(
        session.id,
        session.data,
        next,
        undefined,
        { scheduleSync: shouldScheduleSync },
      );
    }

    if (shouldScheduleSync) {
      scheduleSyncReverseProxyTrustedIPs({
        reason: "session-ip-mobility-enabled",
        delayMs: 0,
      });
    }
  }

  async maintainSessionActiveIps(): Promise<boolean> {
    const settings = await configManager.getAuthCredentialSettings();
    if (!settings.session_ip_mobility_enabled) {
      return false;
    }

    const now = nowSeconds();
    const sessions = await configManager.listSessions();
    let changed = false;
    for (const session of sessions) {
      const removedCount = await this.pruneSessionActiveIps(
        session.id,
        session.data,
        settings,
        now,
        { scheduleSync: false },
      );
      changed = changed || removedCount > 0;
    }

    if (changed) {
      scheduleSyncReverseProxyTrustedIPs({
        reason: "session-active-ip-maintenance",
        delayMs: 50,
      });
    }

    return changed;
  }

  private async recordSessionActiveIp(args: {
    sessionId: string;
    session?: LoginSession | null;
    clientIp: string;
    source: SessionActiveIpSource;
    ipLocation?: string;
    whitelistRecordId?: string | null;
    settings?: AuthCredentialSettings;
    syncReason: string;
    scheduleSync?: boolean;
  }): Promise<SessionActiveIpDetail | null> {
    const settings =
      args.settings ?? (await configManager.getAuthCredentialSettings());
    if (!settings.session_ip_mobility_enabled) {
      return null;
    }

    const normalizedIp =
      normalizeIp(args.clientIp) || String(args.clientIp || "").trim();
    if (!normalizedIp) return null;

    const session =
      args.session ?? (await configManager.getSession(args.sessionId));
    if (!session) return null;

    const windowSeconds = this.getSessionIpMobilityWindowSeconds(settings);
    const now = nowSeconds();
    const sessionExpireAt = toUnixSeconds(session.expiresAt);
    const storageTtl =
      this.resolveProxySessionTTL(sessionExpireAt) ?? windowSeconds;
    if (storageTtl <= 0) return null;

    await this.pruneSessionActiveIps(args.sessionId, session, settings, now, {
      scheduleSync: args.scheduleSync,
    });

    const existing = this.parseActiveIpDetail(
      await this.r.hget(this.activeIpDetailsKey(args.sessionId), normalizedIp),
    );
    const activeExpireAt = Math.min(
      sessionExpireAt ?? now + windowSeconds,
      now + windowSeconds,
    );
    let whitelistRecordId =
      existing?.whitelistRecordId || args.whitelistRecordId || undefined;

    if (this.isFollowSessionAutoGrant(session)) {
      const record = await whitelistManager.ensureSessionAutoWhiteList({
        ownerKey: this.activeIpWhitelistOwnerKey(args.sessionId, normalizedIp),
        ip: normalizedIp,
        expireAt: activeExpireAt,
        comment: session.comment ?? "登录后自动授权",
        existingRecordId: whitelistRecordId,
      });
      whitelistRecordId = record.id;
      await this.r.set(
        this.whitelistOwnerKey(record.id),
        args.sessionId,
        "EX",
        storageTtl,
      );
    } else {
      whitelistRecordId = undefined;
    }

    const detail: SessionActiveIpDetail = {
      version: 1,
      ip: normalizedIp,
      firstSeenAt: existing?.firstSeenAt || now,
      lastSeenAt: now,
      source: args.source,
      ...(args.ipLocation
        ? { ipLocation: args.ipLocation }
        : existing?.ipLocation
          ? { ipLocation: existing.ipLocation }
          : {}),
      ...(whitelistRecordId ? { whitelistRecordId } : {}),
    };

    const pipeline = this.r.pipeline();
    pipeline.zadd(this.activeIpZsetKey(args.sessionId), now, normalizedIp);
    pipeline.hset(
      this.activeIpDetailsKey(args.sessionId),
      normalizedIp,
      JSON.stringify(detail),
    );
    pipeline.expire(this.activeIpZsetKey(args.sessionId), storageTtl);
    pipeline.expire(this.activeIpDetailsKey(args.sessionId), storageTtl);
    await pipeline.exec();

    const removedCount = await this.pruneSessionActiveIps(
      args.sessionId,
      session,
      settings,
      now,
      {
        keepIp: normalizedIp,
        scheduleSync: false,
      },
    );

    if (args.scheduleSync !== false && (!existing || removedCount > 0)) {
      scheduleSyncReverseProxyTrustedIPs({
        reason: args.syncReason,
      });
    }

    return detail;
  }

  private async listSessionActiveIpDetails(
    sessionId: string,
    session: LoginSession | null,
    settings: AuthCredentialSettings,
  ): Promise<SessionActiveIpDetail[]> {
    const windowSeconds = this.getSessionIpMobilityWindowSeconds(settings);
    const now = nowSeconds();
    if (session) {
      await this.pruneSessionActiveIps(sessionId, session, settings, now);
    }

    const activeIps = await this.r.zrangebyscore(
      this.activeIpZsetKey(sessionId),
      now - windowSeconds + 1,
      "+inf",
    );
    if (activeIps.length === 0) return [];

    const raws = await this.r.hmget(
      this.activeIpDetailsKey(sessionId),
      ...activeIps,
    );
    return raws
      .map((raw) => this.parseActiveIpDetail(raw))
      .filter((detail): detail is SessionActiveIpDetail => detail !== null)
      .sort((left, right) => right.lastSeenAt - left.lastSeenAt);
  }

  private async getAllSessionActiveIpDetails(
    sessionId: string,
  ): Promise<SessionActiveIpDetail[]> {
    const details = await this.r.hgetall(this.activeIpDetailsKey(sessionId));
    return Object.values(details)
      .map((raw) => this.parseActiveIpDetail(raw))
      .filter((detail): detail is SessionActiveIpDetail => detail !== null)
      .sort((left, right) => right.lastSeenAt - left.lastSeenAt);
  }

  private parseActiveIpDetail(
    raw: string | null | undefined,
  ): SessionActiveIpDetail | null {
    if (!raw) return null;
    try {
      const parsed = JSON.parse(raw) as Partial<SessionActiveIpDetail>;
      const ip = normalizeIp(parsed.ip || "") || String(parsed.ip || "").trim();
      if (!ip) return null;
      const firstSeenAt = Number.parseInt(String(parsed.firstSeenAt ?? 0), 10);
      const lastSeenAt = Number.parseInt(String(parsed.lastSeenAt ?? 0), 10);
      if (!Number.isFinite(firstSeenAt) || !Number.isFinite(lastSeenAt)) {
        return null;
      }
      const source = this.normalizeActiveIpSource(parsed.source);
      return {
        version: 1,
        ip,
        firstSeenAt,
        lastSeenAt,
        source,
        ...(typeof parsed.ipLocation === "string" && parsed.ipLocation
          ? { ipLocation: parsed.ipLocation }
          : {}),
        ...(typeof parsed.whitelistRecordId === "string" &&
        parsed.whitelistRecordId
          ? { whitelistRecordId: parsed.whitelistRecordId }
          : {}),
      };
    } catch {
      return null;
    }
  }

  private normalizeActiveIpSource(value: unknown): SessionActiveIpSource {
    if (
      value === "login" ||
      value === "proxy-session" ||
      value === "fnos-token" ||
      value === "session-refresh" ||
      value === "browser-session"
    ) {
      return value;
    }
    return "session-refresh";
  }

  private async isSessionActiveAtIp(
    sessionId: string,
    session: LoginSession,
    clientIp: string,
    settings: AuthCredentialSettings,
  ): Promise<boolean> {
    const normalizedIp = normalizeIp(clientIp) || String(clientIp || "").trim();
    if (!normalizedIp) return false;
    const entries = await this.listSessionActiveIpDetails(
      sessionId,
      session,
      settings,
    );
    if (entries.length === 0) {
      return false;
    }
    return entries.some((entry) => entry.ip === normalizedIp);
  }

  private async pruneSessionActiveIps(
    sessionId: string,
    session: LoginSession,
    settings: AuthCredentialSettings,
    now = nowSeconds(),
    options: {
      keepIp?: string;
      scheduleSync?: boolean;
    } = {},
  ): Promise<number> {
    const windowSeconds = this.getSessionIpMobilityWindowSeconds(settings);
    const cutoff = now - windowSeconds;
    const activeIpKey = this.activeIpZsetKey(sessionId);
    const detailKey = this.activeIpDetailsKey(sessionId);
    const normalizedKeepIp = options.keepIp
      ? normalizeIp(options.keepIp) || String(options.keepIp || "").trim()
      : "";
    const [expiredIps, allIps] = await Promise.all([
      this.r.zrangebyscore(activeIpKey, 0, cutoff),
      this.r.zrange(activeIpKey, 0, -1),
    ]);
    const removeIps = new Set(expiredIps);
    const remainingIps = allIps.filter((ip) => !removeIps.has(ip));
    const overflowCount = remainingIps.length - MAX_SESSION_ACTIVE_IPS;
    if (overflowCount > 0) {
      const overflowIps = remainingIps
        .filter((ip) => ip !== normalizedKeepIp)
        .slice(0, overflowCount);
      for (const ip of overflowIps) {
        removeIps.add(ip);
      }
    }

    const ipsToRemove = [...removeIps];
    if (ipsToRemove.length === 0) return 0;

    const raws = await this.r.hmget(detailKey, ...ipsToRemove);
    const details = raws
      .map((raw) => this.parseActiveIpDetail(raw))
      .filter((detail): detail is SessionActiveIpDetail => detail !== null);

    const pipeline = this.r.pipeline();
    pipeline.zrem(activeIpKey, ...ipsToRemove);
    pipeline.hdel(detailKey, ...ipsToRemove);
    await pipeline.exec();

    for (const detail of details) {
      if (detail.whitelistRecordId) {
        await whitelistManager.removeWhiteList(detail.whitelistRecordId);
      }
    }

    if (options.scheduleSync !== false) {
      scheduleSyncReverseProxyTrustedIPs({
        reason: "session-active-ip-pruned",
      });
    }

    const ttl = this.resolveProxySessionTTL(toUnixSeconds(session.expiresAt));
    if (ttl) {
      await Promise.all([
        this.r.expire(activeIpKey, ttl),
        this.r.expire(detailKey, ttl),
      ]);
    }

    return ipsToRemove.length;
  }

  private async cleanupSessionActiveIpState(
    sessionId: string,
    session: LoginSession,
    options: { preserveLegacySingleSlot: boolean },
  ): Promise<void> {
    const details = await this.getAllSessionActiveIpDetails(sessionId);
    let preserveRecordId: string | null = null;

    if (
      options.preserveLegacySingleSlot &&
      this.isFollowSessionAutoGrant(session)
    ) {
      const currentIp =
        normalizeIp(session.ip) || String(session.ip || "").trim();
      const currentDetail = details.find((detail) => detail.ip === currentIp);
      if (currentIp) {
        const record = await whitelistManager.ensureSessionAutoWhiteList({
          ownerKey: this.legacyWhitelistOwnerKey(sessionId),
          ip: currentIp,
          expireAt: toUnixSeconds(session.expiresAt),
          comment: session.comment ?? "登录后自动授权",
          existingRecordId:
            session.postLoginIpGrantRecordId ||
            currentDetail?.whitelistRecordId ||
            undefined,
        });
        preserveRecordId = record.id;
        await this.ensureLegacyProxyBinding({
          sessionId,
          session,
          currentIp,
          whitelistRecordId: record.id,
        });
        if (session.postLoginIpGrantRecordId !== record.id) {
          await configManager.updateSession(sessionId, {
            postLoginIpGrantRecordId: record.id,
          });
        }
      }
    }

    await this.r.del(
      this.activeIpZsetKey(sessionId),
      this.activeIpDetailsKey(sessionId),
    );

    for (const detail of details) {
      if (
        detail.whitelistRecordId &&
        detail.whitelistRecordId !== preserveRecordId
      ) {
        await whitelistManager.removeWhiteList(detail.whitelistRecordId);
      }
    }
  }

  private async ensureLegacyProxyBinding(args: {
    sessionId: string;
    session: LoginSession;
    currentIp: string;
    whitelistRecordId: string;
  }): Promise<void> {
    const expireAt = toUnixSeconds(args.session.expiresAt);
    const ttlSeconds = this.resolveProxySessionTTL(expireAt);
    if (!ttlSeconds) return;

    const existing = await this.getBinding("proxy-session", args.sessionId);
    const nextBinding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: args.currentIp,
          whitelistRecordId: args.whitelistRecordId,
          expireAt,
          lastSeenAt: new Date().toISOString(),
        }
      : this.buildBinding({
          subjectType: "proxy-session",
          subjectKey: args.sessionId,
          currentIp: args.currentIp,
          whitelistRecordId: args.whitelistRecordId,
          expireAt,
          ownerSessionId: args.sessionId,
        });

    const pipeline = this.r.pipeline();
    pipeline.set(
      this.bindingKey("proxy-session", args.sessionId),
      JSON.stringify(nextBinding),
      "EX",
      ttlSeconds,
    );
    pipeline.sadd(
      this.sessionIndexKey(args.sessionId),
      this.bindingKey("proxy-session", args.sessionId),
    );
    pipeline.expire(this.sessionIndexKey(args.sessionId), ttlSeconds);
    pipeline.set(
      this.whitelistOwnerKey(args.whitelistRecordId),
      args.sessionId,
      "EX",
      ttlSeconds,
    );
    await pipeline.exec();
  }

  private isFollowSessionAutoGrant(session: LoginSession): boolean {
    return (
      session.grantType === "login_ip_grant" &&
      session.postLoginIpGrantMode === "follow_session"
    );
  }

  private getSessionIpMobilityWindowSeconds(
    settings: Pick<
      AuthCredentialSettings,
      "session_ip_mobility_window_seconds"
    >,
  ): number {
    const parsed = Number.parseInt(
      String(settings.session_ip_mobility_window_seconds ?? ""),
      10,
    );
    if (!Number.isFinite(parsed)) {
      return DEFAULT_AUTH_CREDENTIAL_SETTINGS.session_ip_mobility_window_seconds;
    }
    return Math.min(24 * 3600, Math.max(60, parsed));
  }

  private buildBinding(args: {
    subjectType: MobilitySubjectType;
    subjectKey: string;
    currentIp: string;
    whitelistRecordId?: string;
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
    source: MobilityDriftSource;
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

  private buildMobilitySummary(
    events: MobilityTimelineEvent[],
  ): SessionMobilitySummary {
    const driftEvents = events.filter(
      (event): event is Extract<MobilityTimelineEvent, { kind: "drift" }> =>
        event.kind === "drift",
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
    return this.remainingSeconds(expireAt);
  }

  private resolveFnosSessionTTL(expiresAt?: string): number | null {
    return this.resolveFnosTTL(toUnixSeconds(expiresAt));
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

  private bindingKey(
    subjectType: MobilitySubjectType,
    subjectKey: string,
  ): string {
    return `${PREFIX}:binding:${subjectType}:${this.hash(subjectType, subjectKey)}`;
  }

  private timelineKey(sessionId: string): string {
    return `${PREFIX}:timeline:${sessionId}`;
  }

  private summaryKey(sessionId: string): string {
    return `${PREFIX}:summary:${sessionId}`;
  }

  private activeIpZsetKey(sessionId: string): string {
    return `${PREFIX}:active_ips:${sessionId}`;
  }

  private activeIpDetailsKey(sessionId: string): string {
    return `${PREFIX}:active_ip_details:${sessionId}`;
  }

  private activeIpWhitelistOwnerKey(sessionId: string, ip: string): string {
    return `auth-mobility:active-ip:${sessionId}:${ip}`;
  }

  private legacyWhitelistOwnerKey(sessionId: string): string {
    return `auth-mobility:legacy:${sessionId}`;
  }

  private sessionIndexKey(sessionId: string): string {
    return `${PREFIX}:session:${sessionId}`;
  }

  private whitelistOwnerKey(whitelistRecordId: string): string {
    return `${PREFIX}:whitelist:${whitelistRecordId}:session`;
  }

  private async getBinding(
    subjectType: MobilitySubjectType,
    subjectKey: string,
  ): Promise<MobilityBinding | null> {
    return this.getBindingByStorageKey(
      this.bindingKey(subjectType, subjectKey),
    );
  }

  private async getBindingByStorageKey(
    storageKey: string,
  ): Promise<MobilityBinding | null> {
    const raw = await this.r.get(storageKey);
    if (!raw) return null;

    try {
      return JSON.parse(raw) as MobilityBinding;
    } catch {
      return null;
    }
  }

  private async getTimelineEvents(
    sessionId: string,
  ): Promise<MobilityTimelineEvent[]> {
    const raw = await this.r.get(this.timelineKey(sessionId));
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

  private async getStoredSummary(
    sessionId: string,
  ): Promise<SessionMobilitySummary | null> {
    const raw = await this.r.get(this.summaryKey(sessionId));
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
    const timelineKey = this.timelineKey(sessionId);
    const summaryKey = this.summaryKey(sessionId);
    const [events, storedSummary, currentTimelineTtl, currentSummaryTtl] =
      await Promise.all([
        this.getTimelineEvents(sessionId),
        this.getStoredSummary(sessionId),
        this.r.ttl(timelineKey),
        this.r.ttl(summaryKey),
      ]);

    const nextEvents = this.limitTimelineEvents(
      events.length === 0 && seedLoginEvent
        ? [seedLoginEvent, event]
        : [...events, event],
    );
    const nextSummary = this.nextSummaryFromEvent(
      events,
      storedSummary,
      event,
      seedLoginEvent,
    );
    const ttlSeconds = this.resolveStorageTTL(
      currentTimelineTtl,
      currentSummaryTtl,
      fallbackTtlSeconds,
    );
    const pipeline = this.r.pipeline();

    if (ttlSeconds) {
      pipeline.set(timelineKey, JSON.stringify(nextEvents), "EX", ttlSeconds);
      pipeline.set(summaryKey, JSON.stringify(nextSummary), "EX", ttlSeconds);
    } else {
      pipeline.set(timelineKey, JSON.stringify(nextEvents));
      pipeline.set(summaryKey, JSON.stringify(nextSummary));
    }

    await pipeline.exec();
  }

  private limitTimelineEvents(
    events: MobilityTimelineEvent[],
  ): MobilityTimelineEvent[] {
    if (events.length <= MAX_TIMELINE_EVENTS) return events;

    const firstEvent = events[0];
    if (firstEvent?.kind === "login") {
      const tailCount = Math.max(0, MAX_TIMELINE_EVENTS - 1);
      return [firstEvent, ...events.slice(-tailCount)];
    }

    return events.slice(-MAX_TIMELINE_EVENTS);
  }

  private nextSummaryFromEvent(
    events: MobilityTimelineEvent[],
    storedSummary: SessionMobilitySummary | null,
    event: MobilityTimelineEvent,
    seedLoginEvent?: MobilityTimelineEvent,
  ): SessionMobilitySummary {
    const baseline =
      storedSummary ??
      this.buildMobilitySummary(
        events.length === 0 && seedLoginEvent ? [seedLoginEvent] : events,
      );

    if (event.kind !== "drift") {
      return baseline;
    }

    return {
      hasHistory: true,
      driftCount: baseline.driftCount + 1,
      lastDriftAt: event.happenedAt,
      lastDriftSource: event.source,
    };
  }

  private resolveStorageTTL(
    ...ttls: Array<number | null | undefined>
  ): number | null {
    const positives = ttls.filter(
      (ttl): ttl is number => typeof ttl === "number" && ttl > 0,
    );
    if (positives.length === 0) return null;
    return Math.max(...positives);
  }

  private async ensureSessionIndexTTL(
    sessionId: string,
    ttlSeconds: number,
  ): Promise<void> {
    const key = this.sessionIndexKey(sessionId);
    const currentTtl = await this.r.ttl(key);
    if (currentTtl < ttlSeconds) {
      await this.r.expire(key, ttlSeconds);
    }
  }
}

export const authMobilitySessionManager = new AuthMobilitySessionManager();
