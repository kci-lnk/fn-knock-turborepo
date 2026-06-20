import type { AppConfig } from "../../redis";
import { getBooleanEnv } from "../../env";
import { resolvePublicAuthBaseUrl } from "../../subdomain-mode";
import { oidcT } from "./messages";

const TRUST_OIDC_FORWARDED_HEADERS =
  getBooleanEnv("OIDC_TRUST_FORWARDED_HEADERS", false) ||
  getBooleanEnv("AUTH_TRUST_FORWARDED_HEADERS", false);

const takeFirstHeaderValue = (value: string | null) => {
  const first = value?.split(",")[0]?.trim();
  return first || "";
};

const normalizeOriginProto = (value: string) =>
  value.trim().replace(/:$/, "").toLowerCase();

const isSafeOriginHost = (host: string) =>
  Boolean(host) && !/[\s,/?#\\@]/.test(host);

const resolveRequestOrigin = (request: Request) => {
  const url = new URL(request.url);
  const requestProto = normalizeOriginProto(url.protocol) || "http";
  const trustedProto = TRUST_OIDC_FORWARDED_HEADERS
    ? takeFirstHeaderValue(request.headers.get("x-forwarded-proto"))
    : "";
  const proto = normalizeOriginProto(trustedProto || requestProto);
  const trustedHost = TRUST_OIDC_FORWARDED_HEADERS
    ? takeFirstHeaderValue(request.headers.get("x-forwarded-host"))
    : "";
  const directHost = request.headers.get("host")?.trim() || url.host;
  const host = trustedHost || directHost;

  if ((proto !== "http" && proto !== "https") || !isSafeOriginHost(host)) {
    throw new Error(oidcT("callbackUrlBuildFailed"));
  }

  return `${proto}://${host}`;
};

const resolveAuthPrefix = (request: Request) => {
  const pathname = new URL(request.url).pathname;
  if (pathname.startsWith("/__auth__/api/auth/")) return "/__auth__";
  if (pathname.startsWith("/auth/api/auth/")) return "/auth";
  return "";
};

const trimTrailingSlash = (value: string) => value.replace(/\/+$/, "");

const resolvePublicAuthAppBaseUrl = (baseUrl: string) => {
  const trimmed = trimTrailingSlash(baseUrl);
  if (!trimmed) return "";
  try {
    const parsed = new URL(trimmed);
    parsed.pathname = parsed.pathname.replace(/\/+$/, "") || "/";
    parsed.search = "";
    parsed.hash = "";
    return trimTrailingSlash(parsed.toString());
  } catch {
    return trimmed;
  }
};

export const buildCallbackUrl = (
  providerId: string,
  request: Request,
  config: AppConfig,
) => {
  const publicBaseUrl = resolvePublicAuthBaseUrl(config);
  if (publicBaseUrl) {
    return `${resolvePublicAuthAppBaseUrl(publicBaseUrl)}/api/auth/oidc/callback/${encodeURIComponent(providerId)}`;
  }
  const prefix = resolveAuthPrefix(request);
  return `${resolveRequestOrigin(request)}${prefix}/api/auth/oidc/callback/${encodeURIComponent(providerId)}`;
};

export const buildInviteUrl = (
  token: string,
  request: Request,
  config: AppConfig,
) => {
  const publicBaseUrl = resolvePublicAuthBaseUrl(config);
  const base = publicBaseUrl
    ? resolvePublicAuthAppBaseUrl(publicBaseUrl)
    : `${resolveRequestOrigin(request)}${resolveAuthPrefix(request)}`;
  return `${base}/api/auth/oidc/bind?token=${encodeURIComponent(token)}`;
};

export const buildOidcDiscoveryUrl = (issuer: string) => {
  const normalized = trimTrailingSlash(issuer);
  return `${normalized}/.well-known/openid-configuration`;
};
