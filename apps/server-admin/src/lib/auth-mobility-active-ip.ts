import { normalizeIp } from "./ip-normalize";
import { DEFAULT_AUTH_CREDENTIAL_SETTINGS } from "./config/defaults";
import type { AuthCredentialSettings } from "./config/types";
import type { MobilityDriftSource } from "./auth-mobility-timeline";

export type SessionActiveIpSource = MobilityDriftSource | "login";

export type SessionActiveIpDetail = {
  version: 1;
  ip: string;
  firstSeenAt: number;
  lastSeenAt: number;
  source: SessionActiveIpSource;
  ipLocation?: string;
  whitelistRecordId?: string;
};

export type SessionActiveIpEntry = {
  ip: string;
  firstSeenAt: string;
  lastSeenAt: string;
  expiresAt: string;
  source: SessionActiveIpSource;
  ipLocation?: string;
  whitelistRecordId?: string;
};

export const MAX_SESSION_ACTIVE_IPS = 32;

export const getSessionIpMobilityWindowSeconds = (
  settings: Pick<AuthCredentialSettings, "session_ip_mobility_window_seconds">,
): number => {
  const parsed = Number.parseInt(
    String(settings.session_ip_mobility_window_seconds ?? ""),
    10,
  );
  if (!Number.isFinite(parsed)) {
    return DEFAULT_AUTH_CREDENTIAL_SETTINGS.session_ip_mobility_window_seconds;
  }
  return Math.min(24 * 3600, Math.max(60, parsed));
};

export const normalizeSessionActiveIpSource = (
  value: unknown,
): SessionActiveIpSource => {
  if (
    value === "login" ||
    value === "proxy-session" ||
    value === "fnos-token" ||
    value === "session-refresh" ||
    value === "browser-session"
  ) {
    return value;
  }
  return "session-refresh";
};

export const parseSessionActiveIpDetail = (
  raw: string | null | undefined,
): SessionActiveIpDetail | null => {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<SessionActiveIpDetail>;
    const ip = normalizeIp(parsed.ip || "") || String(parsed.ip || "").trim();
    if (!ip) return null;
    const firstSeenAt = Number.parseInt(String(parsed.firstSeenAt ?? 0), 10);
    const lastSeenAt = Number.parseInt(String(parsed.lastSeenAt ?? 0), 10);
    if (!Number.isFinite(firstSeenAt) || !Number.isFinite(lastSeenAt)) {
      return null;
    }

    return {
      version: 1,
      ip,
      firstSeenAt,
      lastSeenAt,
      source: normalizeSessionActiveIpSource(parsed.source),
      ...(typeof parsed.ipLocation === "string" && parsed.ipLocation
        ? { ipLocation: parsed.ipLocation }
        : {}),
      ...(typeof parsed.whitelistRecordId === "string" &&
      parsed.whitelistRecordId
        ? { whitelistRecordId: parsed.whitelistRecordId }
        : {}),
    };
  } catch {
    return null;
  }
};

export const toSessionActiveIpEntry = ({
  detail,
  sessionExpireAt,
  windowSeconds,
}: {
  detail: SessionActiveIpDetail;
  sessionExpireAt: number | null;
  windowSeconds: number;
}): SessionActiveIpEntry => {
  const expiresAt = Math.min(
    sessionExpireAt ?? detail.lastSeenAt + windowSeconds,
    detail.lastSeenAt + windowSeconds,
  );
  return {
    ip: detail.ip,
    firstSeenAt: new Date(detail.firstSeenAt * 1000).toISOString(),
    lastSeenAt: new Date(detail.lastSeenAt * 1000).toISOString(),
    expiresAt: new Date(expiresAt * 1000).toISOString(),
    source: detail.source,
    ...(detail.ipLocation ? { ipLocation: detail.ipLocation } : {}),
    ...(detail.whitelistRecordId
      ? { whitelistRecordId: detail.whitelistRecordId }
      : {}),
  };
};
