import { authMobilitySessionManager } from "./auth-mobility-session";
import { fnosShareBypassService } from "./fnos-share-bypass";
import { recentAuthIPsManager } from "./recent-auth-ips";
import { configManager } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { getClientIp } from "./auth-request";

export type AuthAccessDecision = {
  authorized: boolean;
  clientIp: string;
  message: string;
  setCookies: string[];
  responseHeaders: Record<string, string>;
};

export const hasNormalAccessContext = async (
  request: Request,
  clientIp = getClientIp(request),
): Promise<boolean> => {
  if (await whitelistManager.hasValidIP(clientIp)) {
    return true;
  }

  const identity = authMobilitySessionManager.inspectRequest(request);
  if (identity.sessionId) {
    return configManager.isValidSession(identity.sessionId);
  }

  return false;
};

export const resolveAuthAccess = async (
  request: Request,
  clientIp = getClientIp(request),
): Promise<AuthAccessDecision> => {
  if (await whitelistManager.hasValidIP(clientIp)) {
    await authMobilitySessionManager.syncTrustedRequest(request, clientIp);
    await recentAuthIPsManager.recordVerified(clientIp);
    return {
      authorized: true,
      clientIp,
      message: "Authorized by IP whitelist",
      setCookies: [],
      responseHeaders: {},
    };
  }

  const restored = await authMobilitySessionManager.tryRestoreAccess(request, clientIp);
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
