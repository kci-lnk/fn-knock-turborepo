import type { AppConfig, LoginSession } from "./redis";
import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  normalizeLocale,
  translate,
} from "../../../../packages/i18n/src";
import { whitelistManager } from "./whitelist-manager";

export const AUTO_IP_GRANT_COMMENT_KEY = "auth.autoIpGrantComment";
export const LEGACY_AUTO_IP_GRANT_COMMENT_KEY =
  "server.auth.autoIpGrantComment";

export const getAutoIpGrantComment = (locale?: string | null): string =>
  translate(
    normalizeLocale(locale) ?? DEFAULT_LOCALE,
    AUTO_IP_GRANT_COMMENT_KEY,
  );

export const AUTO_IP_GRANT_COMMENT = getAutoIpGrantComment(DEFAULT_LOCALE);

const AUTO_IP_GRANT_COMMENTS = new Set(
  [
    ...SUPPORTED_LOCALES.map((locale) => getAutoIpGrantComment(locale)),
    LEGACY_AUTO_IP_GRANT_COMMENT_KEY,
  ].map((comment) => comment.trim()),
);

export const isAutoIpGrantComment = (
  value: string | null | undefined,
): boolean => AUTO_IP_GRANT_COMMENTS.has(String(value ?? "").trim());

export const normalizeAutoIpGrantComment = (
  value: string | null | undefined,
  locale?: string | null,
): string => {
  const trimmed = String(value ?? "").trim();
  if (!trimmed) return "";
  return isAutoIpGrantComment(trimmed)
    ? getAutoIpGrantComment(locale)
    : trimmed;
};

export const shouldRevokeCustomPostLoginIpGrant = (
  session:
    | Pick<LoginSession, "grantType" | "postLoginIpGrantMode" | "comment">
    | null
    | undefined,
  config: Pick<AppConfig, "auth_credential_settings">,
): boolean => {
  if (!session) return false;

  if (
    session.grantType === "login_ip_grant" &&
    session.postLoginIpGrantMode === "custom"
  ) {
    return true;
  }

  return (
    isAutoIpGrantComment(session.comment) &&
    config.auth_credential_settings?.post_login_ip_grant_mode === "custom"
  );
};

export const revokeCustomPostLoginIpGrant = async (
  session:
    | Pick<
        LoginSession,
        | "grantType"
        | "postLoginIpGrantMode"
        | "comment"
        | "postLoginIpGrantRecordId"
        | "ip"
      >
    | null
    | undefined,
  config: Pick<AppConfig, "auth_credential_settings">,
  fallbackIp?: string | null,
): Promise<boolean> => {
  if (!shouldRevokeCustomPostLoginIpGrant(session, config)) {
    return false;
  }

  if (session?.postLoginIpGrantRecordId) {
    return whitelistManager.removeWhiteList(session.postLoginIpGrantRecordId);
  }

  const ip = session?.ip || fallbackIp;
  if (!ip) {
    return false;
  }

  return whitelistManager.removeRecordsByIP(ip, "auto");
};
