import { authMobilitySessionManager } from "./auth-mobility-session";
import { fnosShareBypassService } from "./fnos-share-bypass";
import { recentAuthIPsManager } from "./recent-auth-ips";
import { configManager } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { getClientIp } from "./auth-request";
import { isWhitelistExemptIp } from "./ip-normalize";

export type RequestedAccessMode = "login_first" | "strict_whitelist";

export type AuthAccessDecision = {
  authorized: boolean;
  clientIp: string;
  message: string;
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
    return configManager.isValidSession(identity.sessionId);
  }

  return false;
};

export const hasWhitelistAccess = async (clientIp: string): Promise<boolean> => {
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
  if (whitelistExempt || (await whitelistManager.hasValidIP(clientIp))) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message: whitelistExempt
        ? "Authorized by local/private IP exemption"
        : "Authorized by IP whitelist",
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

  const restored = await authMobilitySessionManager.tryRestoreAccess(
    request,
    clientIp,
  );
  if (restored.success) {
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message: restored.message || "Authorized",
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
    setCookies: shareAuth.setCookies ?? [],
    responseHeaders: shareAuth.responseHeaders ?? {},
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
