import { authMobilitySessionManager } from "./auth-mobility-session";
import { fnosShareBypassService } from "./fnos-share-bypass";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { recentAuthIPsManager } from "./recent-auth-ips";
import { configManager } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { getClientIp } from "./auth-request";
import { isWhitelistExemptIp } from "./ip-normalize";

export type RequestedAccessMode = "login_first" | "strict_whitelist";
export type AuthGrantType =
  | "local_exempt"
  | "manual_whitelist"
  | "login_ip_grant"
  | "browser_session"
  | "session_migration"
  | "fnos_fingerprint_session"
  | "fnos_share";

export const reliesOnBrowserSessionCookie = (
  grantType?: AuthGrantType,
): boolean =>
  grantType === "browser_session" ||
  grantType === "session_migration";

export type AuthAccessDecision = {
  authorized: boolean;
  clientIp: string;
  message: string;
  grantType?: AuthGrantType;
  setCookies: string[];
  responseHeaders: Record<string, string>;
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
  if (await hasWhitelistAccess(clientIp)) {
    return true;
  }

  if (accessMode === "strict_whitelist") {
    return false;
  }

  const identity = authMobilitySessionManager.inspectRequest(request);
  if (identity.sessionId) {
    if (await configManager.isValidSession(identity.sessionId)) {
      return true;
    }
  }

  if (identity.isFnosApp) {
    return authMobilitySessionManager.hasResolvableFnosSession(
      request,
      clientIp,
    );
  }

  return false;
};

export const hasWhitelistAccess = async (
  clientIp: string,
): Promise<boolean> => {
  if (isWhitelistExemptIp(clientIp)) {
    return true;
  }

  return whitelistManager.hasValidIP(clientIp);
};

export const resolveAuthAccess = async (
  request: Request,
  clientIp = getClientIp(request),
  accessMode = resolveRequestedAccessMode(request),
): Promise<AuthAccessDecision> => {
  const whitelistExempt = isWhitelistExemptIp(clientIp);
  if (whitelistExempt) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message: "Authorized by local/private IP exemption",
      grantType: "local_exempt",
      setCookies: [],
      responseHeaders: {},
    };
  }

  const ipGrantType = await resolveIpGrantType(clientIp);
  if (ipGrantType) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message:
        ipGrantType === "manual_whitelist"
          ? "Authorized by IP whitelist"
          : "Authorized by login IP grant",
      grantType: ipGrantType,
      setCookies: [],
      responseHeaders: {},
    };
  }

  if (accessMode === "strict_whitelist") {
    return {
      authorized: false,
      clientIp,
      message: "Unauthorized by strict whitelist",
      setCookies: [],
      responseHeaders: {},
    };
  }

  const browserSessionDecision = await authorizeBrowserSession(
    request,
    clientIp,
  );
  if (browserSessionDecision.authorized) {
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message: browserSessionDecision.message,
      grantType: browserSessionDecision.grantType,
      setCookies: [],
      responseHeaders: {},
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

const resolveIpGrantType = async (
  clientIp: string,
): Promise<Extract<
  AuthGrantType,
  "manual_whitelist" | "login_ip_grant"
> | null> => {
  const records = await whitelistManager.getActiveRecordsByIP(clientIp);
  if (records.some((record) => record.source === "manual")) {
    return "manual_whitelist";
  }
  if (records.some((record) => record.source === "auto")) {
    return "login_ip_grant";
  }
  return null;
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

  const records = await whitelistManager.getActiveRecordsByIP(session.ip, "auto");
  return records.length === 1 ? records[0]?.id || null : null;
};

const authorizeBrowserSession = async (
  request: Request,
  clientIp: string,
): Promise<
  | {
      authorized: false;
    }
  | {
      authorized: true;
      message: string;
      grantType: Extract<
        AuthGrantType,
        "browser_session" | "session_migration" | "fnos_fingerprint_session"
      >;
    }
> => {
  const restored = await authMobilitySessionManager.tryRestoreAccess(
    request,
    clientIp,
  );
  if (restored.success) {
    return {
      authorized: true,
      message: restored.message || "Authorized",
      grantType: restored.grantType || "browser_session",
    };
  }

  const identity = authMobilitySessionManager.inspectRequest(request);
  if (!identity.sessionId) {
    return { authorized: false };
  }

  const session = await configManager.getSession(identity.sessionId);
  if (!session) {
    return { authorized: false };
  }

  if (session.ip === clientIp) {
    return {
      authorized: true,
      message: "Authorized by browser session",
      grantType: "browser_session",
    };
  }

  const ipLocation = clientIp
    ? await ipLocationService.getCachedLocation(clientIp)
    : "";
  const customGrantRecordId = await resolveCustomGrantRecordId(session);
  await configManager.updateSession(identity.sessionId, {
    ip: clientIp,
    ...(ipLocation ? { ipLocation } : {}),
    ...(customGrantRecordId && !session.postLoginIpGrantRecordId
      ? { postLoginIpGrantRecordId: customGrantRecordId }
      : {}),
  });
  if (clientIp) {
    await ipLocationService.registerUsage(clientIp, [
      ipLocationRefs.session(identity.sessionId),
      ipLocationRefs.sessionTimeline(identity.sessionId),
    ]);
  }
  return {
    authorized: true,
    message: "Authorized by browser session",
    grantType: "browser_session",
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
