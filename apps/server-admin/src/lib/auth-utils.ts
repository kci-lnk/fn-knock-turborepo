import { randomBytes } from "node:crypto";
import { authMobilitySessionManager } from "./auth-mobility-session";
import { resolveClientLocation } from "./auth-request";
import { configManager } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { authLogManager } from "./auth-log";
import { buildSessionCookie } from "./session-cookie";

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

export const getRpInfo = (request: Request) => {
  // Prefer browser-provided headers first when available.
  const fromOrigin = parseAbsoluteUrl(request.headers.get("origin"));
  if (fromOrigin) {
    return { rpID: fromOrigin.hostname, origin: fromOrigin.origin };
  }

  const fromReferer = parseAbsoluteUrl(request.headers.get("referer"));
  if (fromReferer) {
    return { rpID: fromReferer.hostname, origin: fromReferer.origin };
  }

  // run_type=0 proxy mode may strip origin/referer; recover from forwarded/host headers.
  const forwarded = parseForwardedHeader(request.headers.get("forwarded"));
  const forwardedHost =
    forwarded.host ||
    takeFirstHeaderValue(request.headers.get("x-forwarded-host")) ||
    takeFirstHeaderValue(request.headers.get("x-original-host")) ||
    request.headers.get("host");
  if (forwardedHost) {
    const rawProto =
      forwarded.proto ||
      takeFirstHeaderValue(request.headers.get("x-forwarded-proto")) ||
      takeFirstHeaderValue(request.headers.get("x-forwarded-scheme"));
    const requestProto = parseAbsoluteUrl(request.url)?.protocol.replace(":", "");
    const proto =
      rawProto?.trim().replace(/:$/, "") || requestProto || "https";
    const candidate = parseAbsoluteUrl(`${proto}://${forwardedHost}`);
    if (candidate) {
      return { rpID: candidate.hostname, origin: candidate.origin };
    }
  }

  const fallback = parseAbsoluteUrl(request.url) || new URL("http://127.0.0.1");
  return {
    rpID: fallback.hostname,
    origin: fallback.origin,
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

export const buildPasskeyBindInfo = async (totpId: string): Promise<PasskeyBindInfo> => {
  const passkeys = await configManager.getPasskeys();
  const boundPasskeys = passkeys.filter(pk => pk.totpId === totpId);
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
}) => {
  const sessionId = randomBytes(16).toString("hex");
  const maxAge = rememberMe ? 365 * 24 * 3600 : 24 * 3600;

  const clientIpStr = clientIp || "::1";
  const ipLocationStr = await resolveClientLocation(clientIpStr);
  const durationSeconds = rememberMe ? 365 * 24 * 3600 : 24 * 3600;
  const expireAt = Math.floor(Date.now() / 1000) + durationSeconds;
  const expiresAtISO = new Date(expireAt * 1000).toISOString();

  const whitelistRecordId = await whitelistManager.addWhiteList({
    ip: clientIpStr,
    expireAt,
    source: 'auto',
    comment: '登录后自动放行'
  });

  await authLogManager.recordLog({
    type: "login",
    method: authMethod,
    credentialName,
    ip: clientIpStr,
    userAgent,
    success: true,
  });

  await configManager.addSession(sessionId, {
    totpId,
    method: authMethod,
    credentialId,
    credentialName,
    ip: clientIpStr,
    userAgent,
    loginTime: new Date().toISOString(),
    expiresAt: expiresAtISO,
    ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
  }, maxAge);
  await authMobilitySessionManager.registerLoginSession({
    sessionId,
    ip: clientIpStr,
    ...(ipLocationStr ? { ipLocation: ipLocationStr } : {}),
    whitelistRecordId,
    expireAt,
  });
  set.headers["Set-Cookie"] = buildSessionCookie(sessionId, maxAge);
  return {
    success: true,
    message: "Login successful",
    data: { run_type: 1, passkey: passkeyInfo },
  };
};
