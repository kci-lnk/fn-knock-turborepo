import type Redis from "ioredis";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { scheduleSyncReverseProxyTrustedIPs } from "./reverse-proxy-trusted-ips";
import {
  configManager,
  redis,
  type AuthCredentialSettings,
  type LoginSession,
} from "./redis";
import { emitSessionIpDriftEvent } from "./system-events/helpers";
import { normalizeIp } from "./ip-normalize";
import { whitelistManager } from "./whitelist-manager";
import {
  AUTO_IP_GRANT_COMMENT,
  normalizeAutoIpGrantComment,
} from "./post-login-ip-grant";
import {
  listSessionAttachments,
  type SessionFnosAttachment,
  type SessionTrimMediaAttachment,
} from "./auth-mobility-attachments";
import {
  inspectAuthMobilityRequest,
  type AuthMobilityRequestIdentity,
} from "./auth-mobility-identity";
import { authMobilityKeys } from "./auth-mobility-keys";
import {
  AuthMobilityBindingStore,
  buildMobilityBinding,
  type MobilityBinding,
} from "./auth-mobility-binding-store";
import {
  AuthMobilityAppBindingService,
  type AuthMobilityBootstrapOwnerResolution,
} from "./auth-mobility-app-bindings";
import { AuthMobilitySessionCleanupService } from "./auth-mobility-session-cleanup";
import {
  MAX_SESSION_ACTIVE_IPS,
  getSessionIpMobilityWindowSeconds,
  toSessionActiveIpEntry,
  type SessionActiveIpDetail,
  type SessionActiveIpEntry,
  type SessionActiveIpSource,
} from "./auth-mobility-active-ip";
import { AuthMobilityActiveIpStore } from "./auth-mobility-active-ip-store";
import {
  buildMobilityDriftEvent,
  buildMobilityLoginEvent,
  buildMobilitySummary,
  type MobilityDriftSource,
  type MobilityTimelineEvent,
  type SessionMobilityDetails,
  type SessionMobilitySummary,
} from "./auth-mobility-timeline";
import { AuthMobilityTimelineStore } from "./auth-mobility-timeline-store";
import {
  nowSeconds,
  resolveProxySessionTTL,
  toUnixSeconds,
} from "./auth-mobility-time";
import {
  hasResolvableAuthMobilityAccess,
  restoreAuthMobilityAccess,
  type DriftRestoreResult,
} from "./auth-mobility-access-restore";

export type { SessionActiveIpEntry };
export type { SessionMobilityDetails, SessionMobilitySummary };
export type {
  SessionAppAttachment,
  SessionFnosAttachment,
  SessionTrimMediaAttachment,
} from "./auth-mobility-attachments";

export class AuthMobilitySessionManager {
  private readonly r: Redis;
  private readonly bindingStore: AuthMobilityBindingStore;
  private readonly appBindings: AuthMobilityAppBindingService;
  private readonly activeIpStore: AuthMobilityActiveIpStore;
  private readonly timelineStore: AuthMobilityTimelineStore;
  private readonly cleanupService: AuthMobilitySessionCleanupService;

  constructor() {
    this.r = redis;
    this.bindingStore = new AuthMobilityBindingStore(this.r);
    this.appBindings = new AuthMobilityAppBindingService({
      bindingStore: this.bindingStore,
      resolveBootstrapOwner: (clientIp) => this.resolveBootstrapOwner(clientIp),
      resolveSessionOwner: (ownerSessionId) =>
        this.resolveSessionOwner(ownerSessionId),
      syncSessionIp: (args) => this.syncSessionIp(args),
    });
    this.activeIpStore = new AuthMobilityActiveIpStore(this.r);
    this.timelineStore = new AuthMobilityTimelineStore(this.r);
    this.cleanupService = new AuthMobilitySessionCleanupService(
      this.r,
      this.bindingStore,
      this.activeIpStore,
      this.timelineStore,
    );
  }

  inspectRequest(request: Request): AuthMobilityRequestIdentity {
    return inspectAuthMobilityRequest(request);
  }

  async registerLoginSession(args: {
    sessionId: string;
    ip: string;
    ipLocation?: string;
    whitelistRecordId: string;
    expireAt: number | null;
  }): Promise<void> {
    const ttlSeconds = resolveProxySessionTTL(args.expireAt);
    if (!ttlSeconds) return;

    const binding = buildMobilityBinding({
      subjectType: "proxy-session",
      subjectKey: args.sessionId,
      currentIp: args.ip,
      whitelistRecordId: args.whitelistRecordId,
      expireAt: args.expireAt,
      ownerSessionId: args.sessionId,
    });

    const pipeline = this.r.pipeline();
    const loginEvent = buildMobilityLoginEvent({
      ip: args.ip,
      ipLocation: args.ipLocation,
    });
    const proxySessionBindingKey = this.bindingStore.storageKey(
      "proxy-session",
      args.sessionId,
    );
    this.bindingStore.queueSaveWithTtl(
      pipeline,
      proxySessionBindingKey,
      binding,
      ttlSeconds,
    );
    this.timelineStore.queueInitializeSession(pipeline, {
      sessionId: args.sessionId,
      loginEvent,
      ttlSeconds,
    });
    this.bindingStore.queueAddSessionBinding(
      pipeline,
      args.sessionId,
      proxySessionBindingKey,
    );
    this.bindingStore.queueExpireSessionIndex(
      pipeline,
      args.sessionId,
      ttlSeconds,
    );
    pipeline.set(
      authMobilityKeys.whitelistOwner(args.whitelistRecordId),
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
      await this.appBindings.refreshFnosBinding(
        identity.fnosToken,
        clientIp,
        identity.sessionId,
      );
    }

    if (identity.trimMediaToken) {
      await this.appBindings.refreshTrimMediaBinding(
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
    return restoreAuthMobilityAccess(request, clientIp, {
      appBindings: this.appBindings,
      bindingStore: this.bindingStore,
      hasActiveSessionAtIp: (ip) => this.hasActiveSessionAtIp(ip),
      inspectRequest: (nextRequest) => this.inspectRequest(nextRequest),
      resolveBootstrapOwner: (ip) => this.resolveBootstrapOwner(ip),
      resolveSessionOwner: (ownerSessionId) =>
        this.resolveSessionOwner(ownerSessionId),
      restoreAnonymousFnosApp: (ip) => this.restoreAnonymousFnosApp(ip),
      restoreProxySession: (sessionId, ip) =>
        this.restoreProxySession(sessionId, ip),
      restoreTrimMediaApp: (ip) => this.restoreTrimMediaApp(ip),
    });
  }

  async hasResolvableMobilityAccess(
    request: Request,
    clientIp: string,
  ): Promise<boolean> {
    return hasResolvableAuthMobilityAccess(request, clientIp, {
      appBindings: this.appBindings,
      bindingStore: this.bindingStore,
      hasActiveSessionAtIp: (ip) => this.hasActiveSessionAtIp(ip),
      inspectRequest: (nextRequest) => this.inspectRequest(nextRequest),
      resolveBootstrapOwner: (ip) => this.resolveBootstrapOwner(ip),
      resolveSessionOwner: (ownerSessionId) =>
        this.resolveSessionOwner(ownerSessionId),
      restoreAnonymousFnosApp: (ip) => this.restoreAnonymousFnosApp(ip),
      restoreProxySession: (sessionId, ip) =>
        this.restoreProxySession(sessionId, ip),
      restoreTrimMediaApp: (ip) => this.restoreTrimMediaApp(ip),
    });
  }

  async resolveRequestOwnerSessions(
    request: Request,
    clientIp: string,
  ): Promise<AuthMobilityBootstrapOwnerResolution[]> {
    const identity = this.inspectRequest(request);
    const owners = new Map<string, AuthMobilityBootstrapOwnerResolution>();
    const addOwner = async (
      owner: Promise<AuthMobilityBootstrapOwnerResolution | null>,
    ) => {
      const resolved = await owner;
      if (resolved) {
        owners.set(resolved.ownerSessionId, resolved);
      }
    };

    if (identity.sessionId) {
      await addOwner(this.resolveSessionOwner(identity.sessionId));
    }

    if (identity.fnosToken) {
      const binding = await this.bindingStore.get(
        "fnos-token",
        identity.fnosToken,
      );
      if (binding?.ownerSessionId) {
        await addOwner(this.resolveSessionOwner(binding.ownerSessionId));
      }
    }

    if (identity.trimMediaToken) {
      const binding = await this.bindingStore.get(
        "trim-media-token",
        identity.trimMediaToken,
      );
      if (binding?.ownerSessionId) {
        await addOwner(this.resolveSessionOwner(binding.ownerSessionId));
      }
    }

    if (identity.appBinding === "fnos-app") {
      await addOwner(this.resolveBootstrapOwner(clientIp));
    } else if (identity.appBinding === "trim-media-app") {
      for (const owner of await this.listActiveSessionsByIp(clientIp)) {
        owners.set(owner.ownerSessionId, owner);
      }
    }

    return [...owners.values()];
  }

  async destroySession(sessionId: string): Promise<void> {
    await this.cleanupService.destroySession(sessionId);
  }

  async getSessionWhitelistRecordId(sessionId: string): Promise<string | null> {
    return this.cleanupService.getSessionWhitelistRecordId(sessionId);
  }

  async listSessionWhitelistRecordIds(sessionId: string): Promise<string[]> {
    return this.cleanupService.listSessionWhitelistRecordIds(sessionId);
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
      await this.timelineStore.appendEvent({
        sessionId: args.sessionId,
        event: buildMobilityDriftEvent({
          source: args.source,
          fromIp: previousIp,
          fromIpLocation: previousIpLocation,
          toIp: args.clientIp,
          toIpLocation: args.ipLocation,
        }),
        fallbackTtlSeconds: resolveProxySessionTTL(
          toUnixSeconds(nextSession.expiresAt),
        ),
        seedLoginEvent: buildMobilityLoginEvent({
          ip: previousIp,
          ipLocation: previousIpLocation,
          happenedAt: session.loginTime,
        }),
      });
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

    const existing = await this.bindingStore.get("proxy-session", sessionId);
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
    await this.bindingStore.saveKeepTtl(
      this.bindingStore.storageKey("proxy-session", sessionId),
      existing,
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
      this.timelineStore.getSummary(sessionId),
    ]);
    return storedSummary ?? buildMobilitySummary(events);
  }

  async getSessionMobilityDetails(
    sessionId: string,
  ): Promise<SessionMobilityDetails> {
    const session = await configManager.getSession(sessionId);
    const [events, storedSummary] = await Promise.all([
      this.resolveTimelineEvents(sessionId, session),
      this.timelineStore.getSummary(sessionId),
    ]);
    return {
      summary: storedSummary ?? buildMobilitySummary(events),
      events,
    };
  }

  async listSessionFnosAttachments(
    sessionId: string,
  ): Promise<SessionFnosAttachment[]> {
    return listSessionAttachments({
      bindingStore: this.bindingStore,
      sessionId,
      subjectType: "fnos-token",
    });
  }

  async listSessionTrimMediaAttachments(
    sessionId: string,
  ): Promise<SessionTrimMediaAttachment[]> {
    return listSessionAttachments({
      bindingStore: this.bindingStore,
      sessionId,
      subjectType: "trim-media-token",
    });
  }

  private async listActiveSessionsByIp(
    clientIp: string,
  ): Promise<AuthMobilityBootstrapOwnerResolution[]> {
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
        } satisfies AuthMobilityBootstrapOwnerResolution;
      }),
    );
    return resolved.filter(
      (entry): entry is AuthMobilityBootstrapOwnerResolution => entry !== null,
    );
  }

  private async hasActiveSessionAtIp(clientIp: string): Promise<boolean> {
    const sessions = await this.listActiveSessionsByIp(clientIp);
    return sessions.length > 0;
  }

  private async resolveBootstrapOwner(
    clientIp: string,
  ): Promise<AuthMobilityBootstrapOwnerResolution | null> {
    const candidateSessions = await this.listActiveSessionsByIp(clientIp);
    if (candidateSessions.length !== 1) return null;

    const [candidate] = candidateSessions;
    if (!candidate) return null;

    return candidate;
  }

  private async resolveSessionOwner(
    ownerSessionId: string,
  ): Promise<AuthMobilityBootstrapOwnerResolution | null> {
    const ownerSession = await configManager.getSession(ownerSessionId);
    if (!ownerSession) return null;

    return {
      ownerSessionId,
      ownerSession,
    };
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

  private async restoreProxySession(
    sessionId: string,
    clientIp: string,
  ): Promise<boolean> {
    const session = await configManager.getSession(sessionId);
    if (!session) return false;

    let binding = await this.bindingStore.get("proxy-session", sessionId);
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
        await this.bindingStore.saveKeepTtl(
          this.bindingStore.storageKey("proxy-session", sessionId),
          binding,
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
    await this.bindingStore.saveKeepTtl(
      this.bindingStore.storageKey("proxy-session", sessionId),
      binding,
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
    const sessionExpireAt = toUnixSeconds(resolvedSession?.expiresAt);

    return details.map((detail) =>
      toSessionActiveIpEntry({
        detail,
        sessionExpireAt,
        windowSeconds: getSessionIpMobilityWindowSeconds(settings),
      }),
    );
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

    const windowSeconds = getSessionIpMobilityWindowSeconds(settings);
    const now = nowSeconds();
    const sessionExpireAt = toUnixSeconds(session.expiresAt);
    const storageTtl =
      resolveProxySessionTTL(sessionExpireAt) ?? windowSeconds;
    if (storageTtl <= 0) return null;

    await this.pruneSessionActiveIps(args.sessionId, session, settings, now, {
      scheduleSync: args.scheduleSync,
    });

    const existing = await this.activeIpStore.getDetail(
      args.sessionId,
      normalizedIp,
    );
    const activeExpireAt = Math.min(
      sessionExpireAt ?? now + windowSeconds,
      now + windowSeconds,
    );
    let whitelistRecordId =
      existing?.whitelistRecordId || args.whitelistRecordId || undefined;

    if (this.isFollowSessionAutoGrant(session)) {
      const localeConfig = await configManager.getLocaleConfig();
      const record = await whitelistManager.ensureSessionAutoWhiteList({
        ownerKey: authMobilityKeys.activeIpWhitelistOwner(
          args.sessionId,
          normalizedIp,
        ),
        ip: normalizedIp,
        expireAt: activeExpireAt,
        comment:
          normalizeAutoIpGrantComment(
            session.comment,
            localeConfig.default_locale,
          ) || AUTO_IP_GRANT_COMMENT,
        existingRecordId: whitelistRecordId,
      });
      whitelistRecordId = record.id;
      await this.r.set(
        authMobilityKeys.whitelistOwner(record.id),
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

    await this.activeIpStore.saveDetail({
      sessionId: args.sessionId,
      ip: normalizedIp,
      score: now,
      detail,
      ttlSeconds: storageTtl,
    });

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
    const windowSeconds = getSessionIpMobilityWindowSeconds(settings);
    const now = nowSeconds();
    if (session) {
      await this.pruneSessionActiveIps(sessionId, session, settings, now);
    }

    return this.activeIpStore.listRecentDetails({
      sessionId,
      since: now - windowSeconds + 1,
    });
  }

  private async getAllSessionActiveIpDetails(
    sessionId: string,
  ): Promise<SessionActiveIpDetail[]> {
    return this.activeIpStore.listAllDetails(sessionId);
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
    const windowSeconds = getSessionIpMobilityWindowSeconds(settings);
    const cutoff = now - windowSeconds;
    const normalizedKeepIp = options.keepIp
      ? normalizeIp(options.keepIp) || String(options.keepIp || "").trim()
      : "";
    const ipsToRemove = await this.activeIpStore.collectPruneTargets({
      sessionId,
      cutoff,
      keepIp: normalizedKeepIp,
      maxEntries: MAX_SESSION_ACTIVE_IPS,
    });
    if (ipsToRemove.length === 0) return 0;

    const details = await this.activeIpStore.removeIps({
      sessionId,
      ips: ipsToRemove,
    });

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

    const ttl = resolveProxySessionTTL(toUnixSeconds(session.expiresAt));
    if (ttl) {
      await this.activeIpStore.expireSessionKeys(sessionId, ttl);
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
        const localeConfig = await configManager.getLocaleConfig();
        const record = await whitelistManager.ensureSessionAutoWhiteList({
          ownerKey: authMobilityKeys.legacyWhitelistOwner(sessionId),
          ip: currentIp,
          expireAt: toUnixSeconds(session.expiresAt),
          comment:
            normalizeAutoIpGrantComment(
              session.comment,
              localeConfig.default_locale,
            ) || AUTO_IP_GRANT_COMMENT,
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

    await this.activeIpStore.clearSession(sessionId);

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
    const ttlSeconds = resolveProxySessionTTL(expireAt);
    if (!ttlSeconds) return;

    const existing = await this.bindingStore.get(
      "proxy-session",
      args.sessionId,
    );
    const nextBinding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: args.currentIp,
          whitelistRecordId: args.whitelistRecordId,
          expireAt,
          lastSeenAt: new Date().toISOString(),
        }
      : buildMobilityBinding({
          subjectType: "proxy-session",
          subjectKey: args.sessionId,
          currentIp: args.currentIp,
          whitelistRecordId: args.whitelistRecordId,
          expireAt,
          ownerSessionId: args.sessionId,
        });

    const pipeline = this.r.pipeline();
    const proxySessionBindingKey = this.bindingStore.storageKey(
      "proxy-session",
      args.sessionId,
    );
    this.bindingStore.queueSaveWithTtl(
      pipeline,
      proxySessionBindingKey,
      nextBinding,
      ttlSeconds,
    );
    this.bindingStore.queueAddSessionBinding(
      pipeline,
      args.sessionId,
      proxySessionBindingKey,
    );
    this.bindingStore.queueExpireSessionIndex(
      pipeline,
      args.sessionId,
      ttlSeconds,
    );
    pipeline.set(
      authMobilityKeys.whitelistOwner(args.whitelistRecordId),
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

  private async resolveTimelineEvents(
    sessionId: string,
    fallbackSession: LoginSession | null,
  ): Promise<MobilityTimelineEvent[]> {
    const events = await this.timelineStore.getEvents(sessionId);
    if (events.length > 0) return events;
    if (!fallbackSession) return [];
    return [
      buildMobilityLoginEvent({
        ip: fallbackSession.ip,
        ipLocation: fallbackSession.ipLocation,
        happenedAt: fallbackSession.loginTime,
      }),
    ];
  }

}

export const authMobilitySessionManager = new AuthMobilitySessionManager();
