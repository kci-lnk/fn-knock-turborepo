import type { SystemEventEnvelope } from "../../system-events/types";
import { ntfT, withNotificationLocale } from "./context";

const EVENT_LABEL_KEYS: Record<SystemEventEnvelope["type"], string> = {
  FN_EVENT_AUTH_LOGIN_SUCCESS: "events.authLoginSuccess",
  FN_EVENT_AUTH_LOGOUT: "events.authLogout",
  FN_EVENT_AUTH_LOGIN_FAILURE: "events.authLoginFailure",
  FN_EVENT_AUTH_SESSION_IP_DRIFT: "events.authSessionIpDrift",
  FN_EVENT_SECURITY_SCANNER_BLOCKED: "events.securityScannerBlocked",
  FN_EVENT_DDNS_UPDATE_COMPLETED: "events.ddnsUpdateCompleted",
  FN_EVENT_GATEWAY_THROTTLE_BLOCKED: "events.gatewayThrottleBlocked",
  FN_EVENT_WAF_BLOCKED: "events.wafBlocked",
  FN_EVENT_SSH_LOGIN_SUCCESS: "events.sshLoginSuccess",
  FN_EVENT_SSH_LOGIN_FAILURE: "events.sshLoginFailure",
  FN_EVENT_SSH_IP_BLOCKED: "events.sshIpBlocked",
  FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE: "events.appUpdateAvailable",
  FN_EVENT_SYSTEM_CPU_ALERT: "events.cpuAlert",
  FN_EVENT_SYSTEM_CPU_RECOVERED: "events.cpuRecovered",
  FN_EVENT_SYSTEM_MEMORY_ALERT: "events.memoryAlert",
  FN_EVENT_SYSTEM_MEMORY_RECOVERED: "events.memoryRecovered",
  FN_EVENT_TUNNEL_FRP_CONNECTED: "events.frpConnected",
  FN_EVENT_TUNNEL_FRP_DISCONNECTED: "events.frpDisconnected",
  FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED: "events.cloudflaredConnected",
  FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED: "events.cloudflaredDisconnected",
};

const EVENT_LEVEL_LABEL_KEYS: Record<SystemEventEnvelope["level"], string> = {
  INFO: "levels.info",
  WARN: "levels.warn",
  ERROR: "levels.error",
  CRITICAL: "levels.critical",
};

const EVENT_SOURCE_LABEL_KEYS: Record<SystemEventEnvelope["source"], string> = {
  SERVER_ADMIN: "sources.serverAdmin",
  GO_REAUTH_PROXY: "sources.goReauthProxy",
  SYSTEM_MONITOR: "sources.systemMonitor",
};

const AUTH_METHOD_LABEL_KEYS = {
  OIDC: "authMethods.oidc",
} as const;

const GRANT_TYPE_LABEL_KEYS = {
  browser_session: "grantTypes.browserSession",
  login_ip_grant: "grantTypes.loginIpGrant",
} as const;

const WAF_MODE_LABEL_KEYS = {
  detection: "wafModes.detection",
  blocking: "wafModes.blocking",
  off: "wafModes.off",
} as const;

const WAF_ACTION_LABEL_KEYS = {
  block: "wafActions.block",
  deny: "wafActions.deny",
  detect: "wafActions.detect",
  log: "wafActions.log",
  pass: "wafActions.pass",
} as const;

const LOGOUT_SOURCE_LABEL_KEYS = {
  user_logout: "logoutSources.userLogout",
  admin_session_delete: "logoutSources.adminSessionDelete",
} as const;

const DRIFT_SOURCE_LABEL_KEYS = {
  "proxy-session": "driftSources.proxySession",
  "fnos-token": "driftSources.fnosToken",
  "session-refresh": "driftSources.sessionRefresh",
  "browser-session": "driftSources.browserSession",
} as const;

const DDNS_TRIGGER_LABEL_KEYS = {
  cron: "ddnsTriggers.cron",
  enable: "ddnsTriggers.enable",
  startup: "ddnsTriggers.startup",
  manual_test: "ddnsTriggers.manualTest",
} as const;

const DDNS_UPDATE_SCOPE_LABEL_KEYS = {
  ipv4_only: "ddnsUpdateScopes.ipv4Only",
  ipv6_only: "ddnsUpdateScopes.ipv6Only",
} as const;

const DDNS_IP_SOURCE_LABEL_KEYS = {
  public: "ddnsIpSources.public",
  interface: "ddnsIpSources.interface",
  static: "ddnsIpSources.static",
  domain: "ddnsIpSources.domain",
} as const;

const UPDATE_CHECK_REASON_LABEL_KEYS = {
  cron: "updateCheckReasons.cron",
  manual: "updateCheckReasons.manual",
  "manual-check-and-download": "updateCheckReasons.manualCheckAndDownload",
  "download-bootstrap": "updateCheckReasons.downloadBootstrap",
} as const;

export const TUNNEL_LABELS = {
  frp: "FRP",
  cloudflared: "Cloudflared",
} as const;

const translateLabelKey = <T extends string>(
  labels: Partial<Record<T, string>>,
  value: string,
) => {
  const key = labels[value as T];
  return key ? ntfT(key) : value;
};

export const formatNotificationEventLabel = (
  type: SystemEventEnvelope["type"],
  locale?: string | null,
) => {
  const format = () => {
    const key = EVENT_LABEL_KEYS[type];
    return key ? ntfT(key) : type;
  };
  return locale === undefined
    ? format()
    : withNotificationLocale(locale, format);
};

export const buildNotificationRuleName = (
  type: SystemEventEnvelope["type"],
  locale?: string | null,
) =>
  withNotificationLocale(locale, () =>
    ntfT("ruleName", { event: formatNotificationEventLabel(type) }),
  );

export const formatEventLevelLabel = (
  level: SystemEventEnvelope["level"],
) => translateLabelKey(EVENT_LEVEL_LABEL_KEYS, level);

export const formatEventSourceLabel = (
  source: SystemEventEnvelope["source"],
) => translateLabelKey(EVENT_SOURCE_LABEL_KEYS, source);

export const formatAuthMethodLabel = (value: string) => {
  if (value === "TOTP") return "TOTP";
  if (value === "PASSKEY") return "Passkey";
  return translateLabelKey(AUTH_METHOD_LABEL_KEYS, value);
};

export const formatGrantTypeLabel = (value: string) =>
  translateLabelKey(GRANT_TYPE_LABEL_KEYS, value);

export const formatLogoutSourceLabel = (value: string) =>
  translateLabelKey(LOGOUT_SOURCE_LABEL_KEYS, value);

export const formatDriftSourceLabel = (value: string) =>
  translateLabelKey(DRIFT_SOURCE_LABEL_KEYS, value);

export const formatDdnsTriggerLabel = (value: string) =>
  translateLabelKey(DDNS_TRIGGER_LABEL_KEYS, value);

export const formatDdnsUpdateScopeLabel = (value: string) =>
  value === "dual_stack"
    ? "IPv4 + IPv6"
    : translateLabelKey(DDNS_UPDATE_SCOPE_LABEL_KEYS, value);

export const formatDdnsIpSourceLabel = (value: string) =>
  translateLabelKey(DDNS_IP_SOURCE_LABEL_KEYS, value);

export const formatUpdateCheckReasonLabel = (value: string) =>
  translateLabelKey(UPDATE_CHECK_REASON_LABEL_KEYS, value);

export const formatWAFActionLabel = (value: string) =>
  translateLabelKey(WAF_ACTION_LABEL_KEYS, value);

export const formatWAFModeLabel = (value: string) =>
  translateLabelKey(WAF_MODE_LABEL_KEYS, value);

export const isWAFBlockingAction = (action: string, mode: string) => {
  const normalizedAction = action.toLowerCase();
  if (normalizedAction === "block" || normalizedAction === "deny") return true;
  if (
    normalizedAction === "detect" ||
    normalizedAction === "log" ||
    normalizedAction === "pass"
  ) {
    return false;
  }
  return mode.toLowerCase() === "blocking";
};

export const formatWAFOutcomeLabel = (action: string, mode: string) => {
  if (isWAFBlockingAction(action, mode)) return ntfT("wafOutcomeBlocked");
  const actionLabel = formatWAFActionLabel(action);
  return actionLabel || ntfT("wafOutcomeLogged");
};
