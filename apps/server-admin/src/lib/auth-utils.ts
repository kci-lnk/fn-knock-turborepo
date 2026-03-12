import { randomBytes } from "node:crypto";
import { configManager } from "./redis";
import { whitelistManager } from "./whitelist-manager";
import { authLogManager } from "./auth-log";
import { buildSessionCookie } from "./session-cookie";

export type PasskeyBindInfo = {
  available: boolean;
  can_bind: boolean;
  bind_token?: string;
};

export const getRpInfo = (request: Request) => {
  const originHeader = request.headers.get("origin");
  const refererHeader = request.headers.get("referer");
  const targetUrl = originHeader || refererHeader || request.url;
  try {
    const url = new URL(targetUrl);
    return {
      rpID: url.hostname,
      origin: url.origin,
    };
  } catch (e) {
    const fallback = new URL(request.url);
    return {
      rpID: fallback.hostname,
      origin: fallback.origin,
    };
  }
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
  let ipLocationStr = "";
  try {
    const { ipLocationService } = await import("./ip-location");
    const ipAddr = clientIpStr === "::1" ? "127.0.0.1" : clientIpStr;
    const ipInfo = await ipLocationService.getIpLocation(ipAddr);
    if (ipInfo) ipLocationStr = ipInfo.raw;
  } catch {}
  const durationSeconds = rememberMe ? 365 * 24 * 3600 : 24 * 3600;
  const expireAt = Math.floor(Date.now() / 1000) + durationSeconds;
  const expiresAtISO = new Date(expireAt * 1000).toISOString();

  await whitelistManager.addWhiteList({
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
  set.headers["Set-Cookie"] = buildSessionCookie(sessionId, maxAge);
  return {
    success: true,
    message: "Login successful",
    data: { run_type: 1, passkey: passkeyInfo },
  };
};
