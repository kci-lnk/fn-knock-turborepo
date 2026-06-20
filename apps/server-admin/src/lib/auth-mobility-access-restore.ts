import type {
  AuthMobilityAppBindingService,
  AuthMobilityBootstrapOwnerResolution,
} from "./auth-mobility-app-bindings";
import type { AuthMobilityBindingStore } from "./auth-mobility-binding-store";
import type { AuthMobilityRequestIdentity } from "./auth-mobility-identity";
import { resolveFnosSessionTTL } from "./auth-mobility-time";

export type DriftRestoreResult = {
  success: boolean;
  message?: string;
  grantType?: "session_migration" | "fnos_fingerprint_session";
};

type AuthMobilityAccessRestoreDeps = {
  appBindings: AuthMobilityAppBindingService;
  bindingStore: AuthMobilityBindingStore;
  hasActiveSessionAtIp: (clientIp: string) => Promise<boolean>;
  inspectRequest: (request: Request) => AuthMobilityRequestIdentity;
  resolveBootstrapOwner: (
    clientIp: string,
  ) => Promise<AuthMobilityBootstrapOwnerResolution | null>;
  resolveSessionOwner: (
    ownerSessionId: string,
  ) => Promise<AuthMobilityBootstrapOwnerResolution | null>;
  restoreAnonymousFnosApp: (clientIp: string) => Promise<boolean>;
  restoreProxySession: (
    sessionId: string,
    clientIp: string,
  ) => Promise<boolean>;
  restoreTrimMediaApp: (clientIp: string) => Promise<boolean>;
};

export const restoreAuthMobilityAccess = async (
  request: Request,
  clientIp: string,
  deps: AuthMobilityAccessRestoreDeps,
): Promise<DriftRestoreResult> => {
  const identity = deps.inspectRequest(request);

  if (identity.fnosToken) {
    const restored = await deps.appBindings.restoreFnosToken(
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
    const restored = await deps.appBindings.restoreTrimMediaToken(
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
    const restored = await deps.restoreAnonymousFnosApp(clientIp);
    if (restored) {
      return {
        success: true,
        message: "Authorized by fnos app bootstrap session",
        grantType: "fnos_fingerprint_session",
      };
    }
  }

  if (identity.appBinding === "trim-media-app") {
    const restored = await deps.restoreTrimMediaApp(clientIp);
    if (restored) {
      return {
        success: true,
        message: "Authorized by trim media app binding",
        grantType: "fnos_fingerprint_session",
      };
    }
  }

  if (identity.sessionId) {
    const restored = await deps.restoreProxySession(
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
};

export const hasResolvableAuthMobilityAccess = async (
  request: Request,
  clientIp: string,
  deps: AuthMobilityAccessRestoreDeps,
): Promise<boolean> => {
  const identity = deps.inspectRequest(request);
  if (!identity.fnosToken && !identity.trimMediaToken && !identity.appBinding) {
    return false;
  }

  if (identity.fnosToken) {
    const binding = await deps.bindingStore.get(
      "fnos-token",
      identity.fnosToken,
    );
    if (binding?.ownerSessionId) {
      const owner = await deps.resolveSessionOwner(binding.ownerSessionId);
      if (owner) {
        return !!resolveFnosSessionTTL(owner.ownerSession.expiresAt);
      }
    }
  }

  if (identity.trimMediaToken) {
    const binding = await deps.bindingStore.get(
      "trim-media-token",
      identity.trimMediaToken,
    );
    if (binding?.ownerSessionId) {
      const owner = await deps.resolveSessionOwner(binding.ownerSessionId);
      if (owner) {
        return !!resolveFnosSessionTTL(owner.ownerSession.expiresAt);
      }
    }
  }

  if (identity.appBinding === "trim-media-app") {
    return deps.hasActiveSessionAtIp(clientIp);
  }

  if (identity.appBinding === "fnos-app") {
    return !!(await deps.resolveBootstrapOwner(clientIp));
  }

  return false;
};
