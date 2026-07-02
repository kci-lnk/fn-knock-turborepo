import { authMobilitySessionManager } from "./auth-mobility-session";
import { fnosShareBypassService } from "./fnos-share-bypass";
import { ipLocationService } from "./ip-location";
import { recentAuthIPsManager } from "./recent-auth-ips";
import { scheduleCommonAuthLocationsRebuild } from "./common-auth-locations";
import { configManager, type HostMapping, type TOTPCredential } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { whitelistRegionGroupManager } from "./whitelist-region-groups";
import { getClientIp } from "./auth-request";
import { isWhitelistExemptIp } from "./ip-normalize";
import { resolveRequestHostname } from "./subdomain-mode";
import {
  isHostAllowedByTotpSubdomainAccess,
  normalizeSubdomainAccessHost,
  normalizeTotpSubdomainAccess,
  TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE,
  TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH,
} from "./totp-subdomain-access";

export type RequestedAccessMode = "login_first" | "strict_whitelist";
export const REAUTH_ACCESS_DENIED_HEADER = "X-Reauth-Access-Denied" as const;
export const REAUTH_SCOPE_DENIED = "scope" as const;
export const REAUTH_SUBDOMAIN_ACCESS_HEADER =
  "X-Reauth-Subdomain-Access" as const;
export const REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER =
  "X-Reauth-Allowed-Subdomain-Hosts" as const;
export const REAUTH_CREDENTIAL_ID_HEADER = "X-Reauth-Credential-Id" as const;
export const REAUTH_CREDENTIAL_NAME_HEADER =
  "X-Reauth-Credential-Name" as const;
export const REAUTH_CREDENTIAL_METHOD_HEADER =
  "X-Reauth-Credential-Method" as const;
export const REAUTH_LINKED_TOTP_ID_HEADER = "X-Reauth-Linked-Totp-Id" as const;
export const REAUTH_LINKED_TOTP_NAME_HEADER =
  "X-Reauth-Linked-Totp-Name" as const;
const REAUTH_SUBDOMAIN_ACCESS_CUSTOM = "custom";
const AUTH_IDENTITY_HEADER_MAX_LENGTH = 256;
const AUTH_IDENTITY_HEADER_ENCODING_PREFIX = "b64:";

export type AuthGrantType =
  | "local_exempt"
  | "manual_whitelist"
  | "login_ip_grant"
  | "browser_session"
  | "session_migration"
  | "fnos_fingerprint_session"
  | "fnos_share";

export type AuthDenyReason = typeof REAUTH_SCOPE_DENIED;

export const reliesOnBrowserSessionCookie = (
  grantType?: AuthGrantType,
): boolean =>
  grantType === "browser_session" || grantType === "session_migration";

export type AuthAccessDecision = {
  authorized: boolean;
  clientIp: string;
  message: string;
  grantType?: AuthGrantType;
  denyReason?: AuthDenyReason;
  setCookies: string[];
  responseHeaders: Record<string, string>;
};

export type NormalAccessContextDecision = {
  authorized: boolean;
  message: string;
  grantType?: AuthGrantType;
  denyReason?: AuthDenyReason;
  responseHeaders?: Record<string, string>;
};

type SessionSubdomainAccessDecision = {
  protectedHost: boolean;
  allowed: boolean;
  responseHeaders?: Record<string, string>;
};

type MobilitySubdomainAccessDecision = {
  protectedHost: boolean;
  hasOwnerSession: boolean;
  allowed: boolean;
  responseHeaders?: Record<string, string>;
};

type SessionCredentialIdentity = {
  totpId?: unknown;
  method?: unknown;
  credentialId?: unknown;
  credentialName?: unknown;
  linkedTotpName?: unknown;
  comment?: unknown;
};

const resolveForwardedRequestPathname = (request: Request): string => {
  const forwardedPath = request.headers.get("x-forwarded-path")?.trim();
  const rawPath =
    forwardedPath ||
    (() => {
      try {
        return new URL(request.url).pathname;
      } catch {
        return "";
      }
    })();
  if (!rawPath) return "";

  try {
    return new URL(rawPath, "https://fn-knock.internal").pathname;
  } catch {
    return rawPath.split(/[?#]/, 1)[0] || "";
  }
};

const isAuthServiceRequestPathname = (pathname: string): boolean =>
  ["/__auth__", "/auth", "/api/auth"].some(
    (prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`),
  );

export const resolveRequestSubdomainAccessKey = (request: Request): string => {
  const pathname = resolveForwardedRequestPathname(request);
  if (pathname === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH) {
    return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE;
  }
  if (isAuthServiceRequestPathname(pathname)) return "";
  return resolveRequestHostname(request);
};

const buildCredentialSubdomainAccessResponseHeaders = (
  credential: Pick<TOTPCredential, "subdomain_access"> | null | undefined,
): Record<string, string> => {
  if (!credential) return {};
  const access = normalizeTotpSubdomainAccess(credential.subdomain_access);
  if (access.mode !== "custom") return {};

  return {
    [REAUTH_SUBDOMAIN_ACCESS_HEADER]: REAUTH_SUBDOMAIN_ACCESS_CUSTOM,
    [REAUTH_ALLOWED_SUBDOMAIN_HOSTS_HEADER]: access.hosts
      .filter((host) => host !== TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE)
      .join(","),
  };
};

const normalizeCredentialHeaderValue = (value: unknown): string => {
  if (typeof value !== "string") return "";
  return value
    .replace(/[\r\n]+/g, " ")
    .trim()
    .slice(0, AUTH_IDENTITY_HEADER_MAX_LENGTH);
};

const encodeCredentialHeaderValue = (value: string): string =>
  `${AUTH_IDENTITY_HEADER_ENCODING_PREFIX}${Buffer.from(value, "utf8").toString(
    "base64url",
  )}`;

const setCredentialHeader = (
  headers: Record<string, string>,
  key: string,
  value: unknown,
) => {
  const normalized = normalizeCredentialHeaderValue(value);
  if (normalized) headers[key] = encodeCredentialHeaderValue(normalized);
};

const buildSessionCredentialResponseHeaders = (
  session: SessionCredentialIdentity | null | undefined,
): Record<string, string> => {
  if (!session) return {};

  const headers: Record<string, string> = {};
  setCredentialHeader(
    headers,
    REAUTH_CREDENTIAL_ID_HEADER,
    session.credentialId,
  );
  setCredentialHeader(
    headers,
    REAUTH_CREDENTIAL_NAME_HEADER,
    session.credentialName || session.comment,
  );
  setCredentialHeader(headers, REAUTH_CREDENTIAL_METHOD_HEADER, session.method);
  setCredentialHeader(headers, REAUTH_LINKED_TOTP_ID_HEADER, session.totpId);
  setCredentialHeader(
    headers,
    REAUTH_LINKED_TOTP_NAME_HEADER,
    session.linkedTotpName,
  );
  return headers;
};

const NO_STORE_RESPONSE_HEADERS = {
  "Cache-Control": "private, no-store, no-cache, max-age=0, must-revalidate",
  Pragma: "no-cache",
  Expires: "0",
  "CDN-Cache-Control": "private, no-store",
  "Surrogate-Control": "no-store",
} as const;

export const applyNoStoreHeaders = (
  headers: Headers | Record<string, string | number | boolean | undefined>,
) => {
  for (const [key, value] of Object.entries(NO_STORE_RESPONSE_HEADERS)) {
    if (headers instanceof Headers) {
      headers.set(key, value);
      continue;
    }
    headers[key] = value;
  }
};

export const resolveRequestedAccessMode = (
  request: Request,
): RequestedAccessMode => {
  const mode = request.headers
    .get("x-reauth-access-mode")
    ?.trim()
    .toLowerCase();
  return mode === "strict_whitelist" ? "strict_whitelist" : "login_first";
};

export const hasNormalAccessContext = async (
  request: Request,
  clientIp = getClientIp(request),
  accessMode = resolveRequestedAccessMode(request),
): Promise<boolean> => {
  const decision = await resolveNormalAccessContext(
    request,
    clientIp,
    accessMode,
  );
  return decision.authorized;
};

export const resolveNormalAccessContext = async (
  request: Request,
  clientIp = getClientIp(request),
  accessMode = resolveRequestedAccessMode(request),
): Promise<NormalAccessContextDecision> => {
  if (isWhitelistExemptIp(clientIp)) {
    return {
      authorized: true,
      message: "Authorized by local/private IP exemption",
      grantType: "local_exempt",
    };
  }

  const records = await whitelistManager.getActiveRecordsByIP(clientIp);
  if (
    records.some((record) => record.source === "manual") ||
    (await whitelistRegionGroupManager.hasValidIP(clientIp))
  ) {
    return {
      authorized: true,
      message: "Authorized by IP whitelist",
      grantType: "manual_whitelist",
    };
  }

  const browserSessionDecision = await authorizeBrowserSession(
    request,
    clientIp,
  );
  if (accessMode !== "strict_whitelist" && browserSessionDecision.authorized) {
    return browserSessionDecision;
  }
  if (!browserSessionDecision.authorized && browserSessionDecision.denyReason) {
    return {
      authorized: false,
      message: browserSessionDecision.message || "Unauthorized",
      denyReason: browserSessionDecision.denyReason,
      responseHeaders: browserSessionDecision.responseHeaders,
    };
  }

  if (records.some((record) => record.source === "auto")) {
    return {
      authorized: true,
      message: "Authorized by login IP grant",
      grantType: "login_ip_grant",
    };
  }

  if (accessMode === "strict_whitelist") {
    return {
      authorized: false,
      message: "Unauthorized by strict whitelist",
    };
  }

  const identity = authMobilitySessionManager.inspectRequest(request);
  if (identity.fnosToken || identity.trimMediaToken || identity.appBinding) {
    const mobilityScopeDecision = await resolveMobilitySubdomainAccess(
      request,
      clientIp,
    );
    const hasMobilityAccess =
      await authMobilitySessionManager.hasResolvableMobilityAccess(
        request,
        clientIp,
      );
    if (hasMobilityAccess) {
      if (
        mobilityScopeDecision.protectedHost &&
        (!mobilityScopeDecision.hasOwnerSession ||
          !mobilityScopeDecision.allowed)
      ) {
        return {
          authorized: false,
          message: "Access denied by credential scope",
          denyReason: REAUTH_SCOPE_DENIED,
          responseHeaders: mobilityScopeDecision.responseHeaders,
        };
      }
      return {
        authorized: true,
        message: "Authorized by app session binding",
        grantType: "fnos_fingerprint_session",
        responseHeaders: mobilityScopeDecision.responseHeaders,
      };
    }
  }

  return {
    authorized: false,
    message: "Unauthorized",
  };
};

export const hasWhitelistAccess = async (
  clientIp: string,
): Promise<boolean> => {
  if (isWhitelistExemptIp(clientIp)) {
    return true;
  }

  return whitelistManager.hasValidIP(clientIp);
};

const recordRecentVerifiedIP = async (clientIp: string): Promise<void> => {
  await recentAuthIPsManager.recordVerified(clientIp);
  scheduleCommonAuthLocationsRebuild({ reason: "recent-auth-ip" });
};

export const resolveAuthAccess = async (
  request: Request,
  clientIp = getClientIp(request),
  accessMode = resolveRequestedAccessMode(request),
): Promise<AuthAccessDecision> => {
  const whitelistExempt = isWhitelistExemptIp(clientIp);
  if (whitelistExempt) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recordRecentVerifiedIP(clientIp);
    return {
      authorized: true,
      clientIp,
      message: "Authorized by local/private IP exemption",
      grantType: "local_exempt",
      setCookies: [],
      responseHeaders: {},
    };
  }

  const normalAccess = await resolveNormalAccessContext(
    request,
    clientIp,
    accessMode,
  );
  if (normalAccess.authorized) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recordRecentVerifiedIP(clientIp);
    return {
      authorized: true,
      clientIp,
      message: normalAccess.message,
      grantType: normalAccess.grantType,
      setCookies: [],
      responseHeaders: normalAccess.responseHeaders ?? {},
    };
  }
  if (normalAccess.denyReason === REAUTH_SCOPE_DENIED) {
    return {
      authorized: false,
      clientIp,
      message: normalAccess.message,
      denyReason: normalAccess.denyReason,
      setCookies: [],
      responseHeaders: {
        ...(normalAccess.responseHeaders ?? {}),
        [REAUTH_ACCESS_DENIED_HEADER]: REAUTH_SCOPE_DENIED,
      },
    };
  }

  const shareAuth = await fnosShareBypassService.authorize(request);
  return {
    authorized: shareAuth.authorized,
    clientIp,
    message: shareAuth.authorized
      ? "Authorized by fnos share link"
      : "Unauthorized",
    ...(shareAuth.authorized ? { grantType: "fnos_share" as const } : {}),
    setCookies: shareAuth.setCookies ?? [],
    responseHeaders: shareAuth.responseHeaders ?? {},
  };
};

const resolveCustomGrantRecordId = async (
  session: Awaited<ReturnType<typeof configManager.getSession>>,
): Promise<string | null> => {
  if (!session) return null;
  if (session.postLoginIpGrantRecordId) {
    return session.postLoginIpGrantRecordId;
  }
  if (
    session.grantType !== "login_ip_grant" ||
    session.postLoginIpGrantMode !== "custom"
  ) {
    return null;
  }

  const records = await whitelistManager.getActiveRecordsByIP(
    session.ip,
    "auto",
  );
  return records.length === 1 ? records[0]?.id || null : null;
};

const authorizeBrowserSession = async (
  request: Request,
  clientIp: string,
): Promise<
  | {
      authorized: false;
      message?: string;
      denyReason?: AuthDenyReason;
      responseHeaders?: Record<string, string>;
    }
  | {
      authorized: true;
      message: string;
      grantType: Extract<
        AuthGrantType,
        "browser_session" | "session_migration" | "fnos_fingerprint_session"
      >;
      responseHeaders?: Record<string, string>;
    }
> => {
  const identity = authMobilitySessionManager.inspectRequest(request);
  let sessionScopeHeaders: Record<string, string> | undefined;
  if (identity.sessionId) {
    const session = await configManager.getSession(identity.sessionId);
    if (session) {
      const scopeDecision = await resolveSessionSubdomainAccess(
        request,
        session,
      );
      if (!scopeDecision.allowed) {
        return {
          authorized: false,
          message: "Access denied by credential scope",
          denyReason: REAUTH_SCOPE_DENIED,
          responseHeaders: scopeDecision.responseHeaders,
        };
      }
      sessionScopeHeaders = scopeDecision.responseHeaders;
    }
  }
  if (identity.fnosToken || identity.trimMediaToken || identity.appBinding) {
    const scopeDecision = await resolveMobilitySubdomainAccess(
      request,
      clientIp,
    );
    if (
      scopeDecision.protectedHost &&
      scopeDecision.hasOwnerSession &&
      !scopeDecision.allowed
    ) {
      return {
        authorized: false,
        message: "Access denied by credential scope",
        denyReason: REAUTH_SCOPE_DENIED,
        responseHeaders: scopeDecision.responseHeaders,
      };
    }
    sessionScopeHeaders = scopeDecision.responseHeaders;
  }

  const restored = await authMobilitySessionManager.tryRestoreAccess(
    request,
    clientIp,
  );
  if (restored.success) {
    const scopeDecision = await resolveMobilitySubdomainAccess(
      request,
      clientIp,
    );
    if (
      scopeDecision.protectedHost &&
      (!scopeDecision.hasOwnerSession || !scopeDecision.allowed)
    ) {
      return {
        authorized: false,
        message: "Access denied by credential scope",
        denyReason: REAUTH_SCOPE_DENIED,
        responseHeaders: scopeDecision.responseHeaders,
      };
    }
    return {
      authorized: true,
      message: restored.message || "Authorized",
      grantType: restored.grantType || "browser_session",
      responseHeaders: scopeDecision.responseHeaders,
    };
  }

  if (!identity.sessionId) {
    return { authorized: false };
  }

  const session = await configManager.getSession(identity.sessionId);
  if (!session) {
    return { authorized: false };
  }

  if (session.ip === clientIp) {
    if (await authMobilitySessionManager.isSessionIpMobilityEnabled()) {
      await authMobilitySessionManager.syncSessionIp({
        sessionId: identity.sessionId,
        clientIp,
        source: "browser-session",
        ...(session.ipLocation ? { ipLocation: session.ipLocation } : {}),
        syncReason: "browser-session-ip-refresh",
      });
    }
    return {
      authorized: true,
      message: "Authorized by browser session",
      grantType: "browser_session",
      responseHeaders: sessionScopeHeaders,
    };
  }

  const ipLocation = clientIp
    ? await ipLocationService.getCachedLocation(clientIp)
    : "";
  const customGrantRecordId = await resolveCustomGrantRecordId(session);
  await authMobilitySessionManager.syncSessionIp({
    sessionId: identity.sessionId,
    clientIp,
    source: "browser-session",
    ...(ipLocation ? { ipLocation } : {}),
    sessionPatch:
      customGrantRecordId && !session.postLoginIpGrantRecordId
        ? { postLoginIpGrantRecordId: customGrantRecordId }
        : undefined,
    syncReason: "browser-session-ip-update",
  });
  return {
    authorized: true,
    message: "Authorized by browser session",
    grantType: "browser_session",
    responseHeaders: sessionScopeHeaders,
  };
};

export const isProtectedSubdomainAuthHost = ({
  host,
  hostMappings,
}: {
  host: unknown;
  hostMappings: Pick<HostMapping, "host" | "service_role" | "use_auth">[];
}): boolean => {
  const normalizedHost = normalizeSubdomainAccessHost(host);
  if (!normalizedHost) return false;
  if (normalizedHost === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE) return true;

  return hostMappings.some(
    (mapping) =>
      mapping.service_role !== "auth" &&
      mapping.use_auth === true &&
      normalizeSubdomainAccessHost(mapping.host) === normalizedHost,
  );
};

export const resolveTotpCredentialSubdomainHostAccess = ({
  host,
  hostMappings,
  totpId,
  totpCredentials,
}: {
  host: unknown;
  hostMappings: Pick<HostMapping, "host" | "service_role" | "use_auth">[];
  totpId: string;
  totpCredentials: Pick<TOTPCredential, "id" | "subdomain_access">[];
}): SessionSubdomainAccessDecision => {
  const normalizedHost = normalizeSubdomainAccessHost(host);
  if (!normalizedHost) return { protectedHost: false, allowed: true };

  if (
    !isProtectedSubdomainAuthHost({
      host: normalizedHost,
      hostMappings,
    })
  ) {
    return { protectedHost: false, allowed: true };
  }

  const credential = totpCredentials.find((item) => item.id === totpId);
  if (!credential) {
    return { protectedHost: true, allowed: false };
  }

  return {
    protectedHost: true,
    allowed: isHostAllowedByTotpSubdomainAccess({
      access: credential.subdomain_access,
      host: normalizedHost,
    }),
  };
};

const resolveSessionSubdomainAccess = async (
  request: Request,
  session: Awaited<ReturnType<typeof configManager.getSession>>,
): Promise<SessionSubdomainAccessDecision> => {
  if (!session) return { protectedHost: false, allowed: false };

  const [config, totpCredentials] = await Promise.all([
    configManager.getConfig(),
    configManager.getTOTPCredentials(),
  ]);

  const credential = totpCredentials.find((item) => item.id === session.totpId);
  const decision = resolveTotpCredentialSubdomainHostAccess({
    host: resolveRequestSubdomainAccessKey(request),
    hostMappings: config.host_mappings,
    totpId: session.totpId,
    totpCredentials,
  });

  return {
    ...decision,
    responseHeaders: {
      ...buildSessionCredentialResponseHeaders(session),
      ...buildCredentialSubdomainAccessResponseHeaders(credential),
    },
  };
};

const resolveMobilitySubdomainAccess = async (
  request: Request,
  clientIp: string,
): Promise<MobilitySubdomainAccessDecision> => {
  const host = normalizeSubdomainAccessHost(
    resolveRequestSubdomainAccessKey(request),
  );
  if (!host) {
    return { protectedHost: false, hasOwnerSession: false, allowed: true };
  }

  const owners = await authMobilitySessionManager.resolveRequestOwnerSessions(
    request,
    clientIp,
  );
  if (owners.length === 0) {
    const config = await configManager.getConfig();
    const protectedHost = isProtectedSubdomainAuthHost({
      host,
      hostMappings: config.host_mappings,
    });
    return { protectedHost, hasOwnerSession: false, allowed: !protectedHost };
  }

  let protectedHost = false;
  let deniedResponseHeaders: Record<string, string> | undefined;
  for (const owner of owners) {
    const decision = await resolveSessionSubdomainAccess(
      request,
      owner.ownerSession,
    );
    protectedHost = protectedHost || decision.protectedHost;
    if (decision.protectedHost && decision.allowed) {
      return {
        protectedHost: true,
        hasOwnerSession: true,
        allowed: true,
        responseHeaders: decision.responseHeaders,
      };
    }
    if (decision.protectedHost && !decision.allowed) {
      deniedResponseHeaders = deniedResponseHeaders ?? decision.responseHeaders;
    }
  }

  return {
    protectedHost,
    hasOwnerSession: true,
    allowed: !protectedHost,
    responseHeaders: deniedResponseHeaders,
  };
};

export const applyAuthResponseHeaders = (
  set: { headers: Record<string, string | number> },
  decision: Pick<AuthAccessDecision, "setCookies" | "responseHeaders">,
) => {
  const [shareCookie] = decision.setCookies;
  if (shareCookie) {
    set.headers["Set-Cookie"] = shareCookie;
  }

  for (const [key, value] of Object.entries(decision.responseHeaders)) {
    set.headers[key] = value;
  }
};
