export type MobilityAppBinding = "fnos-app" | "trim-media-app";

export type AuthMobilityRequestIdentity = {
  sessionId: string | null;
  fnosToken: string | null;
  trimMediaToken: string | null;
  appBinding: MobilityAppBinding | null;
};

export const parseAuthMobilityCookieValue = (
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

export const parseAuthMobilityHeaderToken = (
  value: string | null,
): string | null => {
  const trimmed = value?.trim();
  if (!trimmed) return null;

  const schemeMatch = trimmed.match(/^(?:bearer|token)\s+(.+)$/i);
  if (schemeMatch?.[1]) {
    const token = schemeMatch[1].trim();
    return token || null;
  }

  return trimmed;
};

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

export const inspectAuthMobilityRequest = (
  request: Request,
): AuthMobilityRequestIdentity => {
  const cookieHeader = request.headers.get("cookie") || "";
  const sessionId = parseAuthMobilityCookieValue(
    cookieHeader,
    "x-go-reauth-proxy-session-id",
  );
  const fnosToken = parseAuthMobilityCookieValue(cookieHeader, "fnos-token");
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
      ? parseAuthMobilityHeaderToken(request.headers.get("authorization")) ||
        parseAuthMobilityHeaderToken(request.headers.get("accesstoken")) ||
        parseAuthMobilityHeaderToken(request.headers.get("access-token"))
      : null;

  return {
    sessionId,
    fnosToken,
    trimMediaToken,
    appBinding,
  };
};
