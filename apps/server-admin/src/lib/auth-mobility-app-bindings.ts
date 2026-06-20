import { ipLocationService } from "./ip-location";
import { configManager, type LoginSession } from "./redis";
import {
  AuthMobilityBindingStore,
  buildMobilityBinding,
  type MobilityBinding,
} from "./auth-mobility-binding-store";
import {
  resolveFnosSessionTTL,
  resolveProxySessionTTL,
  toUnixSeconds,
} from "./auth-mobility-time";
import type { MobilityDriftSource } from "./auth-mobility-timeline";

export type AuthMobilityBootstrapOwnerResolution = {
  ownerSessionId: string;
  ownerSession: LoginSession;
};

type SyncSessionIp = (args: {
  sessionId: string;
  clientIp: string;
  source: MobilityDriftSource;
  ipLocation?: string;
  sessionPatch?: Partial<LoginSession>;
  syncReason: string;
}) => Promise<LoginSession | null>;

export class AuthMobilityAppBindingService {
  constructor(
    private readonly args: {
      bindingStore: AuthMobilityBindingStore;
      resolveBootstrapOwner: (
        clientIp: string,
      ) => Promise<AuthMobilityBootstrapOwnerResolution | null>;
      resolveSessionOwner: (
        ownerSessionId: string,
      ) => Promise<AuthMobilityBootstrapOwnerResolution | null>;
      syncSessionIp: SyncSessionIp;
    },
  ) {}

  async refreshFnosBinding(
    fnosToken: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    await this.refreshAppTokenBinding(
      "fnos-token",
      fnosToken,
      clientIp,
      sessionId,
    );
  }

  async refreshTrimMediaBinding(
    trimMediaToken: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    await this.refreshAppTokenBinding(
      "trim-media-token",
      trimMediaToken,
      clientIp,
      sessionId,
    );
  }

  async restoreFnosToken(fnosToken: string, clientIp: string): Promise<boolean> {
    return this.restoreAppTokenBinding({
      subjectType: "fnos-token",
      subjectKey: fnosToken,
      clientIp,
      syncSource: "fnos-token",
      syncReason: "fnos-token-restore",
    });
  }

  async restoreTrimMediaToken(
    trimMediaToken: string,
    clientIp: string,
  ): Promise<boolean> {
    return this.restoreAppTokenBinding({
      subjectType: "trim-media-token",
      subjectKey: trimMediaToken,
      clientIp,
      syncSource: "fnos-token",
      syncReason: "trim-media-token-restore",
    });
  }

  private async refreshAppTokenBinding(
    subjectType: "fnos-token" | "trim-media-token",
    subjectKey: string,
    clientIp: string,
    sessionId: string | null,
  ): Promise<void> {
    const storageKey = this.args.bindingStore.storageKey(
      subjectType,
      subjectKey,
    );
    let existing = await this.args.bindingStore.get(subjectType, subjectKey);
    if (!sessionId) {
      if (existing?.ownerSessionId) {
        const owner = await this.args.resolveSessionOwner(
          existing.ownerSessionId,
        );
        if (!owner) {
          const orphanedBinding: MobilityBinding = {
            ...existing,
            ownerSessionId: undefined,
            lastSeenAt: new Date().toISOString(),
          };
          await this.args.bindingStore.saveOrphanedBinding({
            storageKey,
            binding: orphanedBinding,
            previousOwnerSessionId: existing.ownerSessionId,
          });
          existing = orphanedBinding;
        } else {
          const ttlSeconds = resolveFnosSessionTTL(
            owner.ownerSession.expiresAt,
          );
          if (!ttlSeconds) return;

          existing.currentIp = clientIp;
          existing.expireAt = toUnixSeconds(owner.ownerSession.expiresAt);
          existing.lastSeenAt = new Date().toISOString();
          await this.args.bindingStore.saveOwnedBinding({
            storageKey,
            binding: existing,
            ownerSessionId: owner.ownerSessionId,
            bindingTtlSeconds: ttlSeconds,
            sessionIndexTtlSeconds:
              resolveProxySessionTTL(
                toUnixSeconds(owner.ownerSession.expiresAt),
              ) || ttlSeconds,
          });
          return;
        }
      }

      const bootstrap = await this.args.resolveBootstrapOwner(clientIp);
      if (!bootstrap) return;

      const { ownerSessionId, ownerSession } = bootstrap;

      const sessionTtl = resolveProxySessionTTL(
        toUnixSeconds(ownerSession.expiresAt),
      );
      const tokenTtl = resolveFnosSessionTTL(ownerSession.expiresAt);
      if (!sessionTtl || !tokenTtl) return;

      const binding: MobilityBinding = existing
        ? {
            ...existing,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : buildMobilityBinding({
            subjectType,
            subjectKey,
            currentIp: clientIp,
            expireAt: toUnixSeconds(ownerSession.expiresAt),
            ownerSessionId,
          });

      await this.args.bindingStore.saveOwnedBinding({
        storageKey,
        binding,
        ownerSessionId,
        bindingTtlSeconds: tokenTtl,
        sessionIndexTtlSeconds: sessionTtl,
      });
      return;
    }

    const session = await configManager.getSession(sessionId);
    if (!session) return;

    if (existing?.ownerSessionId && existing.ownerSessionId !== sessionId) {
      const existingOwner = await configManager.getSession(
        existing.ownerSessionId,
      );
      if (existingOwner) return;
      await this.args.bindingStore.removeSessionBinding(
        existing.ownerSessionId,
        storageKey,
      );
    }

    const ttlSeconds = resolveFnosSessionTTL(session.expiresAt);
    if (!ttlSeconds) return;

    const binding: MobilityBinding = existing
      ? {
          ...existing,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
          lastSeenAt: new Date().toISOString(),
        }
      : buildMobilityBinding({
          subjectType,
          subjectKey,
          currentIp: clientIp,
          expireAt: toUnixSeconds(session.expiresAt),
          ownerSessionId: sessionId,
        });

    const sessionTtl = resolveProxySessionTTL(toUnixSeconds(session.expiresAt));
    await this.args.bindingStore.saveOwnedBinding({
      storageKey,
      binding,
      ownerSessionId: sessionId,
      bindingTtlSeconds: ttlSeconds,
      sessionIndexTtlSeconds: sessionTtl,
    });
  }

  private async restoreAppTokenBinding(args: {
    subjectType: "fnos-token" | "trim-media-token";
    subjectKey: string;
    clientIp: string;
    syncSource: MobilityDriftSource;
    syncReason: string;
  }): Promise<boolean> {
    const storageKey = this.args.bindingStore.storageKey(
      args.subjectType,
      args.subjectKey,
    );
    let binding = await this.args.bindingStore.get(
      args.subjectType,
      args.subjectKey,
    );
    if (binding?.ownerSessionId) {
      const owner = await this.args.resolveSessionOwner(binding.ownerSessionId);
      if (!owner) {
        const orphanedBinding: MobilityBinding = {
          ...binding,
          ownerSessionId: undefined,
          lastSeenAt: new Date().toISOString(),
        };
        await this.args.bindingStore.saveOrphanedBinding({
          storageKey,
          binding: orphanedBinding,
          previousOwnerSessionId: binding.ownerSessionId,
        });
        binding = orphanedBinding;
      }
    }

    if (!binding?.ownerSessionId) {
      const bootstrap = await this.args.resolveBootstrapOwner(args.clientIp);
      if (!bootstrap) return false;

      const ttlSeconds = resolveFnosSessionTTL(
        bootstrap.ownerSession.expiresAt,
      );
      if (!ttlSeconds) return false;

      binding = binding
        ? {
            ...binding,
            currentIp: args.clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
            lastSeenAt: new Date().toISOString(),
          }
        : buildMobilityBinding({
            subjectType: args.subjectType,
            subjectKey: args.subjectKey,
            currentIp: args.clientIp,
            expireAt: toUnixSeconds(bootstrap.ownerSession.expiresAt),
            ownerSessionId: bootstrap.ownerSessionId,
          });

      await this.args.bindingStore.saveOwnedBinding({
        storageKey,
        binding,
        ownerSessionId: bootstrap.ownerSessionId,
        bindingTtlSeconds: ttlSeconds,
        sessionIndexTtlSeconds: resolveProxySessionTTL(
          toUnixSeconds(bootstrap.ownerSession.expiresAt),
        ),
      });
    }

    const ownerSessionId = binding.ownerSessionId;
    if (!ownerSessionId) return false;

    const owner = await this.args.resolveSessionOwner(ownerSessionId);
    if (!owner) return false;
    const ownerSession = owner.ownerSession;

    const ttlSeconds = resolveFnosSessionTTL(ownerSession.expiresAt);
    if (!ttlSeconds) return false;

    const nextIpLocation = args.clientIp
      ? await ipLocationService.getCachedLocation(args.clientIp)
      : "";
    binding.currentIp = args.clientIp;
    binding.expireAt = toUnixSeconds(ownerSession.expiresAt);
    binding.lastSeenAt = new Date().toISOString();
    await this.args.bindingStore.saveWithTtl(storageKey, binding, ttlSeconds);

    const updatedSession = await this.args.syncSessionIp({
      sessionId: ownerSessionId,
      clientIp: args.clientIp,
      source: args.syncSource,
      ...(nextIpLocation ? { ipLocation: nextIpLocation } : {}),
      syncReason: args.syncReason,
    });
    const sessionTtl = resolveProxySessionTTL(
      toUnixSeconds(updatedSession?.expiresAt),
    );
    if (updatedSession && sessionTtl) {
      await this.args.bindingStore.ensureSessionIndexTtl(
        ownerSessionId,
        sessionTtl,
      );
      await this.args.bindingStore.addSessionBinding(ownerSessionId, storageKey);
    }

    return true;
  }
}
