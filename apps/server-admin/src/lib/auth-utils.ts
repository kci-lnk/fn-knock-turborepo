import { randomBytes } from "node:crypto";
import { authMobilitySessionManager } from "./auth-mobility-session";
import { ipLocationRefs, ipLocationService } from "./ip-location";
import { configManager, DEFAULT_AUTH_CREDENTIAL_SETTINGS } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { authLogManager } from "./auth-log";
import { buildSessionCookie } from "./session-cookie";
import {
  resolveCookieDomain,
  resolvePublicAuthBaseUrl,
} from "./subdomain-mode";

export type PasskeyBindInfo = {
  available: boolean;
  can_bind: boolean;
  bind_token?: string;
};

const takeFirstHeaderValue = (value: string | null): string | null => {
  if (!value) return null;
  const first = value.split(",")[0]?.trim();
  return first || null;
};

const parseForwardedHeader = (
  value: string | null,
): { host?: string; proto?: string } => {
  if (!value) return {};
  const firstPart = value.split(",")[0]?.trim();
  if (!firstPart) return {};

  const result: { host?: string; proto?: string } = {};
  for (const segment of firstPart.split(";")) {
    const [rawKey, ...rawValue] = segment.split("=");
    if (!rawKey || rawValue.length === 0) continue;
    const key = rawKey.trim().toLowerCase();
    const val = rawValue.join("=").trim().replace(/^"|"$/g, "");
    if (!val) continue;
    if (key === "host") result.host = val;
    if (key === "proto") result.proto = val;
  }

  return result;
};

const parseAbsoluteUrl = (value: string | null): URL | null => {
  if (!value) return null;
  try {
    return new URL(value);
  } catch {
    return null;
  }
};

const isLoopbackHostname = (hostname: string): boolean => {
  const normalized = hostname.trim().toLowerCase();
  return (
    normalized === "localhost" ||
    normalized === "::1" ||
    normalized === "[::1]" ||
    normalized === "0.0.0.0" ||
    normalized === "[::]" ||
    normalized.startsWith("127.")
  );
};

const buildAbsoluteUrlFromHost = (
  host: string | null,
  proto: string,
): URL | null => {
  if (!host) return null;
  return parseAbsoluteUrl(`${proto}://${host.trim()}`);
};

const normalizeHostLike = (value: string | null | undefined): string =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^[a-z]+:\/\//i, "")
    .replace(/\/.*$/, "")
    .replace(/\.+$/, "");

const pickPreferredUrl = (candidates: Array<URL | null>): URL | null => {
  const urls = candidates.filter(
    (candidate): candidate is URL => candidate instanceof URL,
  );
  return (
    urls.find((candidate) => !isLoopbackHostname(candidate.hostname)) ||
    urls[0] ||
    null
  );
};

const getConfiguredRpHost = async (): Promise<string | null> => {
  const [caHosts, acmeSettings] = await Promise.all([
    configManager.getCAHosts(),
    configManager.getAcmeSettings(),
  ]);

  const configuredHosts = [...caHosts, ...(acmeSettings?.domains || [])]
    .map((value) => value.trim())
    .filter(Boolean);

  return (
    configuredHosts.find((host) => {
      const candidate = buildAbsoluteUrlFromHost(host, "https");
      return candidate && !isLoopbackHostname(candidate.hostname);
    }) || null
  );
};

export const getRpInfo = async (request: Request) => {
  const config = await configManager.getConfig();
  const requestUrl =
    parseAbsoluteUrl(request.url) || new URL("http://127.0.0.1");
  const forwarded = parseForwardedHeader(request.headers.get("forwarded"));
  const rawProto =
    forwarded.proto ||
    takeFirstHeaderValue(request.headers.get("x-forwarded-proto")) ||
    takeFirstHeaderValue(request.headers.get("x-forwarded-scheme"));
  const proto =
    rawProto?.trim().replace(/:$/, "") ||
    requestUrl.protocol.replace(":", "") ||
    "https";

  const forwardedHost =
    forwarded.host ||
    takeFirstHeaderValue(request.headers.get("x-forwarded-host")) ||
    takeFirstHeaderValue(request.headers.get("x-original-host"));

  const directHost =
    takeFirstHeaderValue(request.headers.get("host")) || requestUrl.host;

  const configuredAuthBaseUrl = resolvePublicAuthBaseUrl(config);
  const configuredAuthUrl = parseAbsoluteUrl(configuredAuthBaseUrl);
  const selectedUrl = pickPreferredUrl([
    parseAbsoluteUrl(request.headers.get("origin")),
    parseAbsoluteUrl(request.headers.get("referer")),
    buildAbsoluteUrlFromHost(forwardedHost, proto),
    buildAbsoluteUrlFromHost(directHost, proto),
    configuredAuthUrl,
    requestUrl,
  ]);

  const effectiveOriginUrl =
    selectedUrl ||
    configuredAuthUrl ||
    buildAbsoluteUrlFromHost(directHost, proto) ||
    requestUrl;

  const passkeyMode =
    config.subdomain_mode?.passkey_rp_mode === "parent_domain"
      ? "parent_domain"
      : "auth_host";
  const configuredParentRpId = normalizeHostLike(
    config.subdomain_mode?.passkey_rp_id || config.subdomain_mode?.root_domain,
  );

  if (passkeyMode === "parent_domain" && configuredParentRpId) {
    return {
      rpID: configuredParentRpId,
      origin: effectiveOriginUrl.origin,
      mode: passkeyMode,
    };
  }

  if (selectedUrl && !isLoopbackHostname(selectedUrl.hostname)) {
    return {
      rpID: selectedUrl.hostname,
      origin: effectiveOriginUrl.origin,
      mode: passkeyMode,
    };
  }

  const configuredHost = await getConfiguredRpHost();
  if (configuredHost) {
    const configuredUrl = buildAbsoluteUrlFromHost(configuredHost, proto);
    if (configuredUrl) {
      return {
        rpID: configuredUrl.hostname,
        origin: effectiveOriginUrl.origin,
        mode: passkeyMode,
      };
    }
  }

  return {
    rpID: requestUrl.hostname,
    origin: effectiveOriginUrl.origin,
    mode: passkeyMode,
  };
};

export const bufferToBase64Url = (buffer: Buffer) => {
  return buffer
    .toString("base64")
    .replace(/\+/g, "-")
    .replace(/\//g, "_")
    .replace(/=+$/g, "");
};

export const base64UrlToBuffer = (value: string) => {
  const padding = "=".repeat((4 - (value.length % 4)) % 4);
  const base64 = (value + padding).replace(/-/g, "+").replace(/_/g, "/");
  return Buffer.from(base64, "base64");
};

export const extractChallenge = (clientDataJSON: string): string | null => {
  try {
    const json = JSON.parse(
      base64UrlToBuffer(clientDataJSON).toString("utf-8"),
    );
    return typeof json.challenge === "string" ? json.challenge : null;
  } catch {
    return null;
  }
};

export const buildPasskeyBindInfo = async (
  totpId: string,
): Promise<PasskeyBindInfo> => {
  const passkeys = await configManager.getPasskeys();
  const boundPasskeys = passkeys.filter((pk) => pk.totpId === totpId);
  if (boundPasskeys.length > 0) {
    return { available: true, can_bind: false };
  }
  const token = await configManager.createPasskeyBindToken(totpId);
  return { available: false, can_bind: true, bind_token: token };
};

export const handleLoginSuccess = async ({
  config,
  clientIp,
  userAgent,
  authMethod,
  credentialId,
  credentialName,
  rememberMe,
  set,
  totpId,
  passkeyInfo,
  redirectTo,
}: {
  config: Awaited<ReturnType<typeof configManager.getConfig>>;
  clientIp: string | null;
  userAgent: string;
  authMethod: "TOTP" | "PASSKEY";
  credentialId: string;
  credentialName: string;
  rememberMe: boolean;
  set: any;
  totpId: string;
  passkeyInfo?: PasskeyBindInfo;
  redirectTo?: string | null;
}) => {
  const sessionId = randomBytes(16).toString("hex");
  const credentialSettings =
    config.auth_credential_settings ?? DEFAULT_AUTH_CREDENTIAL_SETTINGS;
  const durationSeconds = rememberMe
    ? credentialSettings.remember_me_ttl_seconds
    : credentialSettings.session_ttl_seconds;
  const maxAge = durationSeconds;

  const clientIpStr = clientIp || "::1";
  const ipLocationStr = await ipLocationService.getCachedLocation(clientIpStr);
  const expireAt = Math.floor(Date.now() / 1000) + durationSeconds;
  const expiresAtISO = new Date(expireAt * 1000).toISOString();

  const whitelistRecordId = await whitelistManager.addWhiteList({
    ip: clientIpStr,
    expireAt,
    source: "auto",
    comment: "登录后自动放行",
  });

  await authLogManager.recordLog({
    type: "login",
    method: authMethod,
    credentialName,
    ip: clientIpStr,
    userAgent,
    success: true,
  });

  await configManager.addSession(
    sessionId,
    {
      totpId,
      method: authMethod,
      credentialId,
      credentialName,
      ip: clientIpStr,
      userAgent,
      loginTime: new Date().toISOString(),
      expiresAt: expiresAtISO,
      ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    },
    maxAge,
  );
  await authMobilitySessionManager.registerLoginSession({
    sessionId,
    ip: clientIpStr,
    ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    whitelistRecordId,
    expireAt,
  });
  await ipLocationService.registerUsage(clientIpStr, [
    ipLocationRefs.session(sessionId),
    ipLocationRefs.sessionTimeline(sessionId),
  ]);
  set.headers["Set-Cookie"] = buildSessionCookie(sessionId, maxAge, {
    domain: resolveCookieDomain(config),
  });
  return {
    success: true,
    message: "Login successful",
    data: {
      run_type: config.run_type,
      passkey: passkeyInfo,
      redirect_to: redirectTo || undefined,
    },
  };
};
