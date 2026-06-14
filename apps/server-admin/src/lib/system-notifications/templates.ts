import type { SystemEventEnvelope } from "../system-events/types";
import { normalizeNotificationMessage } from "./brand";
import type {
  NotificationMessage,
  NotificationMessageFact,
  NotificationRule,
  NotificationSeverity,
} from "./types";
import {
  DEFAULT_LOCALE,
  type LocaleCode,
  normalizeLocale,
  translate,
} from "../../../../../packages/i18n/src";
import { normalizeAutoIpGrantComment } from "../post-login-ip-grant";

let activeNotificationLocale: LocaleCode = DEFAULT_LOCALE;

const withNotificationLocale = <T>(
  locale: string | null | undefined,
  action: () => T,
): T => {
  const previous = activeNotificationLocale;
  activeNotificationLocale = normalizeLocale(locale) ?? DEFAULT_LOCALE;
  try {
    return action();
  } finally {
    activeNotificationLocale = previous;
  }
};

const ntfT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string =>
  translate(
    activeNotificationLocale,
    `server.notifications.templates.${key}`,
    params,
  );

const detailT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
): string => ntfT(`details.${key}`, params);

const factLabel = (key: string): string => detailT(`facts.${key}`);

const formatSeconds = (value: string) =>
  value ? detailT("units.seconds", { count: value }) : "";

const formatMinutes = (value: string) =>
  value ? detailT("units.minutes", { count: value }) : "";

const formatTimes = (value: string) =>
  value ? detailT("units.times", { count: value }) : "";

const formatRatePerSecond = (value: string) =>
  value ? detailT("units.ratePerSecond", { count: value }) : "";

const joinLocalizedList = (items: string[]) =>
  items.filter(Boolean).join(detailT("listSeparator"));

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

const APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH = 360;

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
  manual_test: "ddnsTriggers.manualTest",
} as const;

const DDNS_UPDATE_SCOPE_LABEL_KEYS = {
  ipv4_only: "ddnsUpdateScopes.ipv4Only",
  ipv6_only: "ddnsUpdateScopes.ipv6Only",
} as const;

const DDNS_IP_SOURCE_LABEL_KEYS = {
  public: "ddnsIpSources.public",
  interface: "ddnsIpSources.interface",
} as const;

const UPDATE_CHECK_REASON_LABEL_KEYS = {
  cron: "updateCheckReasons.cron",
  manual: "updateCheckReasons.manual",
  "manual-check-and-download": "updateCheckReasons.manualCheckAndDownload",
  "download-bootstrap": "updateCheckReasons.downloadBootstrap",
} as const;

const TUNNEL_LABELS = {
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

const formatAuthMethodLabel = (value: string) => {
  if (value === "TOTP") return "TOTP";
  if (value === "PASSKEY") return "Passkey";
  return translateLabelKey(AUTH_METHOD_LABEL_KEYS, value);
};

const formatGrantTypeLabel = (value: string) =>
  translateLabelKey(GRANT_TYPE_LABEL_KEYS, value);

const formatLogoutSourceLabel = (value: string) =>
  translateLabelKey(LOGOUT_SOURCE_LABEL_KEYS, value);

const formatDriftSourceLabel = (value: string) =>
  translateLabelKey(DRIFT_SOURCE_LABEL_KEYS, value);

const formatDdnsTriggerLabel = (value: string) =>
  translateLabelKey(DDNS_TRIGGER_LABEL_KEYS, value);

const formatDdnsUpdateScopeLabel = (value: string) =>
  value === "dual_stack"
    ? "IPv4 + IPv6"
    : translateLabelKey(DDNS_UPDATE_SCOPE_LABEL_KEYS, value);

const formatDdnsIpSourceLabel = (value: string) =>
  translateLabelKey(DDNS_IP_SOURCE_LABEL_KEYS, value);

const formatUpdateCheckReasonLabel = (value: string) =>
  translateLabelKey(UPDATE_CHECK_REASON_LABEL_KEYS, value);

const readPayloadValue = (event: SystemEventEnvelope, key: string) => {
  const payload = event.payload as Record<string, unknown>;
  const value = payload[key];
  if (value === undefined || value === null || value === "") return "";
  return String(value);
};

const readSessionComment = (event: SystemEventEnvelope): string =>
  normalizeAutoIpGrantComment(
    readPayloadValue(event, "session_comment"),
    activeNotificationLocale,
  );

const joinCompactParts = (...parts: Array<string | undefined>) =>
  parts
    .map((part) => String(part || "").trim())
    .filter(Boolean)
    .join(" | ");

const formatCredentialContext = (event: SystemEventEnvelope, fallback = "") => {
  const credentialName = readPayloadValue(event, "credential_name");
  const linkedTotpName = readPayloadValue(event, "linked_totp_name");
  const authMethod = formatAuthMethodLabel(
    readPayloadValue(event, "auth_method"),
  );

  if (linkedTotpName) {
    return ntfT("credentialLinkedTotp", {
      authMethod: authMethod || ntfT("credential"),
      credential: credentialName || ntfT("unknownCredential"),
      totp: linkedTotpName,
    });
  }
  if (credentialName) {
    return ntfT("credentialName", { credential: credentialName });
  }
  return fallback;
};

const formatSessionCommentCompact = (value: string) =>
  value
    ? ntfT("sessionCommentCompact", {
        comment: normalizeAutoIpGrantComment(value, activeNotificationLocale),
      })
    : "";

const appendSessionComment = (text: string, sessionComment: string) =>
  sessionComment
    ? ntfT("appendSessionComment", {
        text,
        comment: normalizeAutoIpGrantComment(
          sessionComment,
          activeNotificationLocale,
        ),
      })
    : text;

const formatEventLevelLabel = (level: SystemEventEnvelope["level"]) =>
  translateLabelKey(EVENT_LEVEL_LABEL_KEYS, level);

const formatEventSourceLabel = (source: SystemEventEnvelope["source"]) =>
  translateLabelKey(EVENT_SOURCE_LABEL_KEYS, source);

const formatDateTime = (value: string) => {
  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) {
    return String(value || "").trim();
  }

  return new Intl.DateTimeFormat(activeNotificationLocale, {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
    hour12: false,
  })
    .format(date)
    .replaceAll("/", "-");
};

const formatBoolean = (value: string) => {
  if (value === "true") return ntfT("yes");
  if (value === "false") return ntfT("no");
  return value;
};

const formatIpTransition = (previousIp: string, nextIp: string) => {
  if (previousIp && nextIp) return `${previousIp} -> ${nextIp}`;
  return previousIp || nextIp;
};

const formatWAFActionLabel = (value: string) =>
  translateLabelKey(WAF_ACTION_LABEL_KEYS, value);

const formatWAFModeLabel = (value: string) =>
  translateLabelKey(WAF_MODE_LABEL_KEYS, value);

const isWAFBlockingAction = (action: string, mode: string) => {
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

const formatWAFOutcomeLabel = (action: string, mode: string) => {
  if (isWAFBlockingAction(action, mode)) return ntfT("wafOutcomeBlocked");
  const actionLabel = formatWAFActionLabel(action);
  return actionLabel || ntfT("wafOutcomeLogged");
};

const truncateText = (value: string, maxLength = 180) => {
  const normalized = value.replace(/\s+/g, " ").trim();
  if (!normalized) return "";
  if (normalized.length <= maxLength) return normalized;
  return `${normalized.slice(0, maxLength).trim()}...`;
};

const pushFact = (
  facts: NotificationMessageFact[],
  label: string,
  value: string | undefined,
) => {
  const normalized = String(value || "").trim();
  if (!normalized) return;
  facts.push({ label, value: normalized });
};

const buildBodyText = (args: {
  overview: string;
  aggregation?: string;
  advice?: string;
}) =>
  [
    args.overview ? `${args.overview}` : "",
    args.aggregation ? `${args.aggregation}` : "",
    args.advice ? `${args.advice}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

const buildBodyMarkdown = (args: {
  overview: string;
  aggregation?: string;
  advice?: string;
}) =>
  [
    args.overview ? `**${ntfT("sections.overview")}**\n${args.overview}` : "",
    args.aggregation
      ? `**${ntfT("sections.aggregation")}**\n${args.aggregation}`
      : "",
    args.advice ? `**${ntfT("sections.advice")}**\n${args.advice}` : "",
  ]
    .filter(Boolean)
    .join("\n\n");

const buildAggregationText = (matchedCount: number, windowSeconds: number) =>
  matchedCount > 1
    ? ntfT("aggregationText", { count: matchedCount, seconds: windowSeconds })
    : "";

const getScannerPaths = (event: SystemEventEnvelope) => {
  const hits = (event.payload as Record<string, unknown>).hits;
  if (!Array.isArray(hits)) return [];

  return hits
    .map((item) => {
      if (!item || typeof item !== "object") return "";
      return String((item as { path?: string }).path || "").trim();
    })
    .filter(Boolean);
};

const buildNotificationDetails = (args: {
  event: SystemEventEnvelope;
  rule: NotificationRule;
  matchedCount: number;
}) => {
  const { event, rule, matchedCount } = args;
  const facts: NotificationMessageFact[] = [];
  const aggregation = buildAggregationText(matchedCount, rule.window_seconds);

  let summary =
    formatEventSummary(event) || formatNotificationEventLabel(event.type);
  let overview = summary;
  let advice = "";

  switch (event.type) {
    case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
      const credentialName =
        readPayloadValue(event, "credential_name") || ntfT("unknownCredential");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readSessionComment(event);
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const ipLocation = readPayloadValue(event, "ip_location");
      const authMethodRaw = readPayloadValue(event, "auth_method");
      const authProviderName = readPayloadValue(event, "auth_provider_name");
      const authMethod = formatAuthMethodLabel(authMethodRaw);
      const isOidcLogin = authMethodRaw === "OIDC";
      const loginMethodText =
        isOidcLogin && authProviderName
          ? detailT("authLoginSuccess.loginViaProvider", {
              provider: authProviderName,
            })
          : detailT("authLoginSuccess.loginWithMethod", {
              method: authMethod || detailT("unknownMethod"),
            });
      const loginAuthText =
        isOidcLogin && authProviderName
          ? detailT("authLoginSuccess.authViaProvider", {
              provider: authProviderName,
            })
          : detailT("authLoginSuccess.authWithMethod", {
              method: authMethod || detailT("unknownMethod"),
            });
      const grantType = formatGrantTypeLabel(
        readPayloadValue(event, "grant_type"),
      );
      const rememberMe = formatBoolean(readPayloadValue(event, "remember_me"));
      const expiresAt = formatDateTime(readPayloadValue(event, "expires_at"));

      summary = appendSessionComment(
        isOidcLogin
          ? detailT("authLoginSuccess.summaryOidc", {
              credential: credentialName,
              method: loginMethodText,
              ip,
              totpPart: linkedTotpName
                ? detailT("authLoginSuccess.linkedTotpPart", {
                    totp: linkedTotpName,
                  })
                : "",
            })
          : linkedTotpName
            ? detailT("authLoginSuccess.summaryTotp", {
                method: authMethod || ntfT("credential"),
                credential: credentialName,
                totp: linkedTotpName,
                ip,
              })
            : detailT("authLoginSuccess.summaryCredential", {
                credential: credentialName,
                ip,
              }),
        sessionComment,
      );
      overview = detailT("authLoginSuccess.overview", {
        auth: loginAuthText,
        grantType: grantType || detailT("unknown"),
        locationPart: ipLocation
          ? detailT("authLoginSuccess.locationPart", { location: ipLocation })
          : "",
        commentPart: sessionComment
          ? detailT("sessionCommentSentence", { comment: sessionComment })
          : "",
      });
      advice = detailT("authLoginSuccess.advice");

      pushFact(facts, factLabel("credentialName"), credentialName);
      pushFact(facts, factLabel("linkedTotp"), linkedTotpName);
      pushFact(facts, factLabel("sessionComment"), sessionComment);
      pushFact(facts, factLabel("loginIp"), ip);
      pushFact(facts, factLabel("ipLocation"), ipLocation);
      pushFact(facts, factLabel("authMethod"), authMethod);
      pushFact(facts, factLabel("loginProvider"), authProviderName);
      pushFact(facts, factLabel("grantType"), grantType);
      pushFact(facts, factLabel("rememberLogin"), rememberMe);
      pushFact(facts, factLabel("sessionExpiresAt"), expiresAt);
      pushFact(
        facts,
        factLabel("sessionId"),
        readPayloadValue(event, "session_id"),
      );
      break;
    }
    case "FN_EVENT_AUTH_LOGOUT": {
      const credentialName =
        readPayloadValue(event, "credential_name") || ntfT("unknownCredential");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readSessionComment(event);
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const ipLocation = readPayloadValue(event, "ip_location");
      const authMethod = formatAuthMethodLabel(
        readPayloadValue(event, "auth_method"),
      );
      const logoutSource = formatLogoutSourceLabel(
        readPayloadValue(event, "logout_source"),
      );

      summary = appendSessionComment(
        linkedTotpName
          ? detailT("authLogout.summaryTotp", {
              method: authMethod || ntfT("credential"),
              credential: credentialName,
              totp: linkedTotpName,
            })
          : detailT("authLogout.summaryCredential", {
              credential: credentialName,
            }),
        sessionComment,
      );
      overview = detailT("authLogout.overview", {
        ip,
        locationPart: ipLocation
          ? detailT("parenthesized", { value: ipLocation })
          : "",
        source: logoutSource || detailT("unknown"),
        commentPart: sessionComment
          ? detailT("sessionCommentSentence", { comment: sessionComment })
          : "",
      });
      advice = detailT("authLogout.advice");

      pushFact(facts, factLabel("credentialName"), credentialName);
      pushFact(facts, factLabel("linkedTotp"), linkedTotpName);
      pushFact(facts, factLabel("sessionComment"), sessionComment);
      pushFact(facts, factLabel("loginIp"), ip);
      pushFact(facts, factLabel("ipLocation"), ipLocation);
      pushFact(facts, factLabel("logoutSource"), logoutSource);
      pushFact(
        facts,
        factLabel("loginTime"),
        formatDateTime(readPayloadValue(event, "login_time")),
      );
      pushFact(
        facts,
        factLabel("sessionId"),
        readPayloadValue(event, "session_id"),
      );
      break;
    }
    case "FN_EVENT_AUTH_LOGIN_FAILURE": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const attempts = readPayloadValue(event, "attempts") || "0";
      const retryAfter = readPayloadValue(event, "retry_after_seconds");
      const blockedUntil = formatDateTime(
        readPayloadValue(event, "blocked_until"),
      );
      const method = formatAuthMethodLabel(readPayloadValue(event, "method"));
      const credentialName = readPayloadValue(event, "credential_name");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");

      summary = detailT("authLoginFailure.summary", { ip, attempts });
      overview = detailT("authLoginFailure.overview", {
        ip,
        retryPart: retryAfter
          ? detailT("authLoginFailure.retryPart", { seconds: retryAfter })
          : "",
        blockedPart: blockedUntil
          ? detailT("authLoginFailure.blockedPart", { time: blockedUntil })
          : "",
      });
      advice = detailT("authLoginFailure.advice");

      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("failureAttempts"), formatTimes(attempts));
      pushFact(facts, factLabel("authMethod"), method);
      pushFact(facts, factLabel("credentialName"), credentialName);
      pushFact(facts, factLabel("linkedTotp"), linkedTotpName);
      pushFact(facts, factLabel("retryWait"), formatSeconds(retryAfter));
      pushFact(facts, factLabel("limitUntil"), blockedUntil);
      break;
    }
    case "FN_EVENT_AUTH_SESSION_IP_DRIFT": {
      const credentialName = readPayloadValue(event, "credential_name");
      const linkedTotpName = readPayloadValue(event, "linked_totp_name");
      const sessionComment = readSessionComment(event);
      const authMethod = formatAuthMethodLabel(
        readPayloadValue(event, "auth_method"),
      );
      const fromIp = readPayloadValue(event, "from_ip") || detailT("unknownIp");
      const toIp = readPayloadValue(event, "to_ip") || detailT("unknownIp");
      const source = formatDriftSourceLabel(
        readPayloadValue(event, "drift_source"),
      );
      const sessionLabel = formatCredentialContext(
        event,
        detailT("currentSession"),
      );

      summary = appendSessionComment(
        detailT("authSessionIpDrift.summary", {
          session: sessionLabel,
          fromIp,
          toIp,
        }),
        sessionComment,
      );
      overview = detailT("authSessionIpDrift.overview", {
        session: sessionLabel,
        source: source || detailT("unknown"),
        commentPart: sessionComment
          ? detailT("sessionCommentSentence", { comment: sessionComment })
          : "",
      });
      advice = detailT("authSessionIpDrift.advice");

      pushFact(facts, factLabel("credentialName"), credentialName);
      pushFact(facts, factLabel("linkedTotp"), linkedTotpName);
      pushFact(facts, factLabel("sessionComment"), sessionComment);
      pushFact(facts, factLabel("authMethod"), authMethod);
      pushFact(facts, factLabel("originalIp"), fromIp);
      pushFact(
        facts,
        factLabel("originalLocation"),
        readPayloadValue(event, "from_ip_location"),
      );
      pushFact(facts, factLabel("currentIp"), toIp);
      pushFact(
        facts,
        factLabel("currentLocation"),
        readPayloadValue(event, "to_ip_location"),
      );
      pushFact(facts, factLabel("driftSource"), source);
      pushFact(
        facts,
        factLabel("loginTime"),
        formatDateTime(readPayloadValue(event, "login_time")),
      );
      pushFact(
        facts,
        factLabel("sessionId"),
        readPayloadValue(event, "session_id"),
      );
      break;
    }
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const windowMinutes = readPayloadValue(event, "window_minutes") || "0";
      const hitCount = readPayloadValue(event, "hit_count") || "0";
      const threshold = readPayloadValue(event, "threshold") || "0";
      const scannerPaths = getScannerPaths(event).slice(0, 3);

      summary = detailT("securityScannerBlocked.summary", { ip });
      overview = detailT("securityScannerBlocked.overview", {
        minutes: windowMinutes,
        hits: hitCount,
        threshold,
        pathsPart:
          scannerPaths.length > 0
            ? detailT("securityScannerBlocked.pathsPart", {
                paths: joinLocalizedList(scannerPaths),
              })
            : "",
      });
      advice = detailT("securityScannerBlocked.advice");

      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(
        facts,
        factLabel("ipLocation"),
        readPayloadValue(event, "ip_location"),
      );
      pushFact(facts, factLabel("hitCount"), formatTimes(hitCount));
      pushFact(
        facts,
        factLabel("observationWindow"),
        formatMinutes(windowMinutes),
      );
      pushFact(facts, factLabel("triggerThreshold"), formatTimes(threshold));
      pushFact(
        facts,
        factLabel("blockedAt"),
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      pushFact(
        facts,
        factLabel("recentPaths"),
        joinLocalizedList(scannerPaths),
      );
      break;
    }
    case "FN_EVENT_DDNS_UPDATE_COMPLETED": {
      const targetName =
        readPayloadValue(event, "target_name") ||
        readPayloadValue(event, "domain_summary") ||
        detailT("ddnsUpdateCompleted.defaultTarget");
      const provider =
        readPayloadValue(event, "provider") || detailT("unknownProvider");
      const success = readPayloadValue(event, "success") === "true";
      const resultMessage = readPayloadValue(event, "message");
      const trigger = formatDdnsTriggerLabel(
        readPayloadValue(event, "trigger"),
      );
      const updateScope = formatDdnsUpdateScopeLabel(
        readPayloadValue(event, "update_scope"),
      );
      const ipSource = formatDdnsIpSourceLabel(
        readPayloadValue(event, "ip_source"),
      );
      const ipv4Change = formatIpTransition(
        readPayloadValue(event, "previous_ipv4"),
        readPayloadValue(event, "next_ipv4"),
      );
      const ipv6Change = formatIpTransition(
        readPayloadValue(event, "previous_ipv6"),
        readPayloadValue(event, "next_ipv6"),
      );

      summary = detailT(
        success
          ? "ddnsUpdateCompleted.summarySuccess"
          : "ddnsUpdateCompleted.summaryFailure",
        { target: targetName },
      );
      overview = detailT("ddnsUpdateCompleted.overview", {
        trigger: trigger || detailT("ddnsUpdateCompleted.currentTask"),
        scope: updateScope || detailT("unknown"),
        ipSource: ipSource || detailT("unknown"),
        resultPart: resultMessage
          ? detailT("ddnsUpdateCompleted.resultPart", {
              message: resultMessage,
            })
          : "",
      });
      advice = success
        ? detailT("ddnsUpdateCompleted.adviceSuccess")
        : detailT("ddnsUpdateCompleted.adviceFailure");

      pushFact(facts, factLabel("target"), targetName);
      pushFact(facts, factLabel("provider"), provider);
      pushFact(
        facts,
        factLabel("targetType"),
        readPayloadValue(event, "is_primary") === "true"
          ? detailT("ddnsUpdateCompleted.primaryDomain")
          : detailT("ddnsUpdateCompleted.additionalDomain"),
      );
      pushFact(facts, factLabel("trigger"), trigger);
      pushFact(facts, factLabel("updateScope"), updateScope);
      pushFact(facts, factLabel("ipSource"), ipSource);
      pushFact(facts, factLabel("ipv4Change"), ipv4Change);
      pushFact(facts, factLabel("ipv6Change"), ipv6Change);
      pushFact(facts, factLabel("result"), resultMessage);
      break;
    }
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const blockSeconds = readPayloadValue(event, "block_seconds") || "0";
      const requestsPerSecond =
        readPayloadValue(event, "requests_per_second") || "0";
      const burst = readPayloadValue(event, "burst") || "0";
      const host = readPayloadValue(event, "host");
      const path = readPayloadValue(event, "path");

      summary = detailT("gatewayThrottleBlocked.summary", {
        ip,
        seconds: blockSeconds,
      });
      overview = detailT("gatewayThrottleBlocked.overview", {
        rate: requestsPerSecond,
        burst,
        targetPart:
          host || path
            ? detailT("gatewayThrottleBlocked.targetPart", {
                target: joinCompactParts(host, path),
              })
            : "",
      });
      advice = detailT("gatewayThrottleBlocked.advice");

      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("blockDuration"), formatSeconds(blockSeconds));
      pushFact(
        facts,
        factLabel("blockedUntil"),
        formatDateTime(readPayloadValue(event, "blocked_until")),
      );
      pushFact(
        facts,
        factLabel("rateLimit"),
        formatRatePerSecond(requestsPerSecond),
      );
      pushFact(facts, factLabel("burstCapacity"), burst);
      pushFact(facts, factLabel("targetHost"), host);
      pushFact(facts, factLabel("requestPath"), path);
      pushFact(
        facts,
        factLabel("routeType"),
        readPayloadValue(event, "route_type"),
      );
      pushFact(
        facts,
        factLabel("authRoute"),
        formatBoolean(readPayloadValue(event, "is_auth_route")),
      );
      break;
    }
    case "FN_EVENT_WAF_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const host = readPayloadValue(event, "host");
      const path =
        readPayloadValue(event, "request_uri") ||
        readPayloadValue(event, "path");
      const ruleIds = readPayloadValue(event, "rule_ids");
      const traceId = readPayloadValue(event, "trace_id");
      const action = readPayloadValue(event, "action");
      const mode = readPayloadValue(event, "mode");
      const actionLabel = formatWAFActionLabel(action);
      const modeLabel = formatWAFModeLabel(mode);
      const outcomeLabel = formatWAFOutcomeLabel(action, mode);
      const isBlocking = isWAFBlockingAction(action, mode);

      summary = detailT("wafBlocked.summary", { ip, outcome: outcomeLabel });
      overview = detailT("wafBlocked.overview", {
        outcome: outcomeLabel,
        ip,
        hostPart: host ? detailT("wafBlocked.hostPart", { host }) : "",
        pathPart: path ? detailT("wafBlocked.pathPart", { path }) : "",
        actionPart: actionLabel
          ? detailT("wafBlocked.actionPart", { action: actionLabel })
          : "",
        modePart: modeLabel
          ? detailT("wafBlocked.modePart", { mode: modeLabel })
          : "",
        rulesPart: ruleIds
          ? detailT("wafBlocked.rulesPart", { rules: ruleIds })
          : "",
      });
      advice = isBlocking
        ? detailT("wafBlocked.adviceBlocked")
        : detailT("wafBlocked.adviceLogged");

      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("traceId"), traceId);
      pushFact(facts, "Host", host);
      pushFact(facts, factLabel("requestAddress"), path);
      pushFact(facts, factLabel("outcome"), outcomeLabel);
      pushFact(facts, factLabel("wafAction"), actionLabel);
      pushFact(facts, factLabel("wafMode"), modeLabel);
      pushFact(facts, factLabel("ruleIds"), ruleIds);
      pushFact(
        facts,
        factLabel("ruleBundle"),
        readPayloadValue(event, "bundle_id"),
      );
      pushFact(
        facts,
        factLabel("statusCode"),
        readPayloadValue(event, "status"),
      );
      pushFact(
        facts,
        factLabel("blockedAt"),
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      break;
    }
    case "FN_EVENT_SSH_LOGIN_SUCCESS": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const ipLocation = readPayloadValue(event, "ip_location");
      const username =
        readPayloadValue(event, "username") || detailT("unknownUser");
      const authMethod = readPayloadValue(event, "auth_method");

      summary = detailT("sshLoginSuccess.summary", { username, ip });
      overview = detailT("sshLoginSuccess.overview", {
        ip,
        locationPart: ipLocation
          ? detailT("parenthesized", { value: ipLocation })
          : "",
        authPart: authMethod
          ? detailT("sshLoginSuccess.authPart", { authMethod })
          : "",
      });
      advice = detailT("sshLoginSuccess.advice");

      pushFact(facts, factLabel("user"), username);
      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("ipLocation"), ipLocation);
      pushFact(facts, factLabel("authMethod"), authMethod);
      pushFact(facts, factLabel("port"), readPayloadValue(event, "port"));
      pushFact(
        facts,
        factLabel("logTime"),
        formatDateTime(readPayloadValue(event, "log_time")),
      );
      break;
    }
    case "FN_EVENT_SSH_LOGIN_FAILURE": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const ipLocation = readPayloadValue(event, "ip_location");
      const username =
        readPayloadValue(event, "username") || detailT("unknownUser");
      const attempts = readPayloadValue(event, "attempts") || "0";
      const threshold = readPayloadValue(event, "threshold") || "0";
      const windowMinutes = readPayloadValue(event, "window_minutes") || "0";

      summary = detailT("sshLoginFailure.summary", { username, ip });
      overview = detailT("sshLoginFailure.overview", {
        minutes: windowMinutes,
        attempts,
        threshold,
        locationPart: ipLocation
          ? detailT("sshLoginFailure.locationPart", { location: ipLocation })
          : "",
      });
      advice = detailT("sshLoginFailure.advice");

      pushFact(facts, factLabel("user"), username);
      pushFact(
        facts,
        factLabel("invalidUser"),
        formatBoolean(readPayloadValue(event, "invalid_user")),
      );
      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("ipLocation"), ipLocation);
      pushFact(
        facts,
        factLabel("authMethod"),
        readPayloadValue(event, "auth_method"),
      );
      pushFact(facts, factLabel("port"), readPayloadValue(event, "port"));
      pushFact(facts, factLabel("failureAttempts"), attempts);
      pushFact(facts, factLabel("threshold"), threshold);
      pushFact(facts, factLabel("window"), formatMinutes(windowMinutes));
      break;
    }
    case "FN_EVENT_SSH_IP_BLOCKED": {
      const ip = readPayloadValue(event, "ip") || detailT("unknownIp");
      const ipLocation = readPayloadValue(event, "ip_location");
      const reason = readPayloadValue(event, "reason");
      const reasonLabel =
        reason === "cidr_not_allowed"
          ? detailT("sshIpBlocked.reasonCidrNotAllowed")
          : detailT("sshIpBlocked.reasonFailedThreshold");

      summary = detailT("sshIpBlocked.summary", { ip });
      overview = detailT("sshIpBlocked.overview", {
        ip,
        locationPart: ipLocation
          ? detailT("parenthesized", { value: ipLocation })
          : "",
        reason: reasonLabel,
      });
      advice = detailT("sshIpBlocked.advice");

      pushFact(facts, factLabel("sourceIp"), ip);
      pushFact(facts, factLabel("ipLocation"), ipLocation);
      pushFact(facts, factLabel("blockedReason"), reasonLabel);
      pushFact(
        facts,
        factLabel("relatedUser"),
        readPayloadValue(event, "username"),
      );
      pushFact(
        facts,
        factLabel("failureAttempts"),
        readPayloadValue(event, "failed_count"),
      );
      pushFact(
        facts,
        factLabel("window"),
        formatMinutes(readPayloadValue(event, "window_minutes")),
      );
      pushFact(
        facts,
        factLabel("threshold"),
        readPayloadValue(event, "threshold"),
      );
      pushFact(
        facts,
        factLabel("blockedAt"),
        formatDateTime(readPayloadValue(event, "blocked_at")),
      );
      pushFact(
        facts,
        factLabel("blockedUntil"),
        formatDateTime(readPayloadValue(event, "blocked_until")),
      );
      break;
    }
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE": {
      const localVersion =
        readPayloadValue(event, "local_version") ||
        detailT("appUpdateAvailable.currentVersionUnknown");
      const latestVersion =
        readPayloadValue(event, "latest_version") ||
        detailT("appUpdateAvailable.targetVersionUnknown");
      const forceUpdate = readPayloadValue(event, "force_update") === "true";
      const checkReason = formatUpdateCheckReasonLabel(
        readPayloadValue(event, "check_reason"),
      );
      const releaseNotes = truncateText(
        readPayloadValue(event, "release_notes"),
        APP_UPDATE_RELEASE_NOTES_PREVIEW_LENGTH,
      );

      summary = detailT("appUpdateAvailable.summary", {
        version: latestVersion,
      });
      overview = detailT("appUpdateAvailable.overview", {
        reason: checkReason || detailT("appUpdateAvailable.currentCheck"),
        localVersion,
        latestVersion,
        forcePart: forceUpdate ? detailT("appUpdateAvailable.forcePart") : "",
      });
      advice = releaseNotes
        ? detailT("appUpdateAvailable.releaseNotesAdvice", {
            releaseNotes,
          })
        : detailT("appUpdateAvailable.advice");

      pushFact(facts, factLabel("currentVersion"), localVersion);
      pushFact(facts, factLabel("latestVersion"), latestVersion);
      pushFact(facts, factLabel("checkReason"), checkReason);
      pushFact(
        facts,
        factLabel("forceUpdate"),
        forceUpdate ? ntfT("yes") : ntfT("no"),
      );
      pushFact(facts, factLabel("releaseNotes"), releaseNotes);
      break;
    }
    case "FN_EVENT_SYSTEM_CPU_ALERT":
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED": {
      const isCpuEvent =
        event.type === "FN_EVENT_SYSTEM_CPU_ALERT" ||
        event.type === "FN_EVENT_SYSTEM_CPU_RECOVERED";
      const recovered =
        event.type === "FN_EVENT_SYSTEM_CPU_RECOVERED" ||
        event.type === "FN_EVENT_SYSTEM_MEMORY_RECOVERED";
      const metricLabel = isCpuEvent ? "CPU" : detailT("memoryMetric");
      const hostname =
        readPayloadValue(event, "hostname") || detailT("unknownHost");
      const usagePercent = readPayloadValue(event, "usage_percent") || "0";
      const thresholdPercent =
        readPayloadValue(event, "threshold_percent") || "0";
      const recoverPercent = readPayloadValue(event, "recover_percent") || "0";

      summary = recovered
        ? detailT("systemMetric.recoveredSummary", {
            hostname,
            metric: metricLabel,
            usage: usagePercent,
          })
        : detailT("systemMetric.alertSummary", {
            hostname,
            metric: metricLabel,
            usage: usagePercent,
          });
      overview = recovered
        ? detailT("systemMetric.recoveredOverview", {
            hostname,
            metric: metricLabel,
            usage: usagePercent,
            recover: recoverPercent,
            threshold: thresholdPercent,
          })
        : detailT("systemMetric.alertOverview", {
            hostname,
            metric: metricLabel,
            usage: usagePercent,
            threshold: thresholdPercent,
            recover: recoverPercent,
          });
      advice = recovered
        ? detailT("systemMetric.recoveredAdvice")
        : detailT("systemMetric.alertAdvice");

      pushFact(facts, factLabel("hostname"), hostname);
      pushFact(facts, factLabel("currentUsage"), `${usagePercent}%`);
      pushFact(facts, factLabel("alertThreshold"), `${thresholdPercent}%`);
      pushFact(facts, factLabel("recoverThreshold"), `${recoverPercent}%`);
      pushFact(
        facts,
        factLabel("sampleInterval"),
        formatSeconds(readPayloadValue(event, "sample_interval_seconds")),
      );
      pushFact(
        facts,
        factLabel("sustainDuration"),
        formatSeconds(readPayloadValue(event, "sustain_seconds")),
      );
      break;
    }
    case "FN_EVENT_TUNNEL_FRP_CONNECTED":
    case "FN_EVENT_TUNNEL_FRP_DISCONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED": {
      const tunnel =
        TUNNEL_LABELS[
          readPayloadValue(event, "tunnel") as keyof typeof TUNNEL_LABELS
        ] || (event.type.includes("CLOUDFLARED") ? "Cloudflared" : "FRP");
      const connected = readPayloadValue(event, "status") === "connected";
      const message = truncateText(readPayloadValue(event, "message"), 200);
      const pid = readPayloadValue(event, "pid");

      summary = detailT(
        connected ? "tunnel.connectedSummary" : "tunnel.disconnectedSummary",
        { tunnel },
      );
      overview = connected
        ? detailT("tunnel.connectedOverview", {
            tunnel,
            messagePart: message
              ? detailT("tunnel.connectedMessagePart", { message })
              : "",
          })
        : detailT("tunnel.disconnectedOverview", {
            tunnel,
            messagePart: message
              ? detailT("tunnel.disconnectedMessagePart", { message })
              : "",
          });
      advice = connected
        ? detailT("tunnel.connectedAdvice")
        : detailT("tunnel.disconnectedAdvice");

      pushFact(facts, factLabel("tunnelType"), tunnel);
      pushFact(
        facts,
        factLabel("connectionStatus"),
        connected ? detailT("connected") : detailT("disconnected"),
      );
      pushFact(facts, factLabel("processPid"), pid);
      pushFact(facts, factLabel("runtimeFeedback"), message);
      break;
    }
  }

  pushFact(
    facts,
    factLabel("eventType"),
    formatNotificationEventLabel(event.type),
  );
  pushFact(facts, factLabel("riskLevel"), formatEventLevelLabel(event.level));
  pushFact(
    facts,
    factLabel("eventSource"),
    formatEventSourceLabel(event.source),
  );
  pushFact(facts, factLabel("happenedAt"), formatDateTime(event.happened_at));
  pushFact(
    facts,
    factLabel("aggregationStats"),
    matchedCount > 1
      ? detailT("aggregationStatsValue", {
          count: matchedCount,
          seconds: rule.window_seconds,
        })
      : "",
  );

  return {
    summary,
    body_text: buildBodyText({
      overview,
      aggregation,
      advice,
    }),
    body_markdown: buildBodyMarkdown({
      overview,
      aggregation,
      advice,
    }),
    facts,
  };
};

const formatEventSummary = (event: SystemEventEnvelope) => {
  switch (event.type) {
    case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
      const authMethod = readPayloadValue(event, "auth_method");
      const authProviderName = readPayloadValue(event, "auth_provider_name");
      if (authMethod === "OIDC" && authProviderName) {
        return joinCompactParts(
          detailT("authLoginSuccess.loginViaProvider", {
            provider: authProviderName,
          }),
          readPayloadValue(event, "credential_name") ||
            ntfT("unknownCredential"),
          formatSessionCommentCompact(readSessionComment(event)),
          readPayloadValue(event, "ip"),
        );
      }
      return joinCompactParts(
        readPayloadValue(event, "credential_name") || ntfT("unknownCredential"),
        formatSessionCommentCompact(readSessionComment(event)),
        readPayloadValue(event, "ip"),
      );
    }
    case "FN_EVENT_AUTH_LOGOUT":
      return joinCompactParts(
        readPayloadValue(event, "credential_name") || ntfT("unknownCredential"),
        formatSessionCommentCompact(readSessionComment(event)),
        readPayloadValue(event, "ip"),
      );
    case "FN_EVENT_AUTH_LOGIN_FAILURE":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "attempts")
          ? detailT("short.loginFailureAttempts", {
              count: readPayloadValue(event, "attempts"),
            })
          : "",
      );
    case "FN_EVENT_AUTH_SESSION_IP_DRIFT":
      return joinCompactParts(
        formatCredentialContext(event),
        formatSessionCommentCompact(readSessionComment(event)),
        formatIpTransition(
          readPayloadValue(event, "from_ip"),
          readPayloadValue(event, "to_ip"),
        ),
      );
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "hit_count")
          ? detailT("short.scanHits", {
              count: readPayloadValue(event, "hit_count"),
            })
          : detailT("short.scanBlocked"),
      );
    case "FN_EVENT_DDNS_UPDATE_COMPLETED":
      return joinCompactParts(
        readPayloadValue(event, "target_name") ||
          readPayloadValue(event, "domain_summary") ||
          readPayloadValue(event, "provider"),
        readPayloadValue(event, "success") === "true"
          ? detailT("short.success")
          : detailT("short.failure"),
      );
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "block_seconds")
          ? detailT("short.blockSeconds", {
              seconds: readPayloadValue(event, "block_seconds"),
            })
          : detailT("short.blockTriggered"),
      );
    case "FN_EVENT_WAF_BLOCKED": {
      const outcomeLabel = formatWAFOutcomeLabel(
        readPayloadValue(event, "action"),
        readPayloadValue(event, "mode"),
      );
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "host"),
        `WAF ${outcomeLabel}`,
        readPayloadValue(event, "rule_ids")
          ? detailT("short.rules", {
              rules: readPayloadValue(event, "rule_ids"),
            })
          : "",
      );
    }
    case "FN_EVENT_SSH_LOGIN_SUCCESS":
      return joinCompactParts(
        readPayloadValue(event, "username"),
        readPayloadValue(event, "ip"),
        detailT("short.sshLoginSuccess"),
      );
    case "FN_EVENT_SSH_LOGIN_FAILURE":
      return joinCompactParts(
        readPayloadValue(event, "username"),
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "attempts")
          ? detailT("short.loginFailureAttempts", {
              count: readPayloadValue(event, "attempts"),
            })
          : detailT("short.sshLoginFailure"),
      );
    case "FN_EVENT_SSH_IP_BLOCKED":
      return joinCompactParts(
        readPayloadValue(event, "ip"),
        readPayloadValue(event, "reason") === "cidr_not_allowed"
          ? detailT("short.regionNotAllowed")
          : detailT("short.failureThreshold"),
      );
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE":
      return joinCompactParts(
        readPayloadValue(event, "latest_version"),
        readPayloadValue(event, "local_version")
          ? detailT("short.currentVersion", {
              version: readPayloadValue(event, "local_version"),
            })
          : "",
      );
    case "FN_EVENT_SYSTEM_CPU_ALERT":
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED":
      return joinCompactParts(
        readPayloadValue(event, "hostname"),
        readPayloadValue(event, "usage_percent")
          ? `${readPayloadValue(event, "usage_percent")}%`
          : "",
      );
    case "FN_EVENT_TUNNEL_FRP_CONNECTED":
    case "FN_EVENT_TUNNEL_FRP_DISCONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED":
      return joinCompactParts(
        TUNNEL_LABELS[
          readPayloadValue(event, "tunnel") as keyof typeof TUNNEL_LABELS
        ] || (event.type.includes("CLOUDFLARED") ? "Cloudflared" : "FRP"),
        readPayloadValue(event, "status") === "connected"
          ? detailT("connected")
          : detailT("disconnected"),
      );
    default:
      return "";
  }
};

const buildNotificationTitle = (
  event: SystemEventEnvelope,
  matchedCount: number,
) => {
  const driftCredentialName = readPayloadValue(event, "credential_name");
  const baseTitle =
    event.type === "FN_EVENT_DDNS_UPDATE_COMPLETED"
      ? readPayloadValue(event, "success") === "true"
        ? detailT("titles.ddnsUpdateSuccess", {
            target:
              readPayloadValue(event, "target_name") ||
              readPayloadValue(event, "domain_summary") ||
              "DDNS",
          })
        : detailT("titles.ddnsUpdateFailure", {
            target:
              readPayloadValue(event, "target_name") ||
              readPayloadValue(event, "domain_summary") ||
              "DDNS",
          })
      : event.type === "FN_EVENT_AUTH_SESSION_IP_DRIFT"
        ? driftCredentialName
          ? detailT("titles.credentialIpDrift", {
              credential: driftCredentialName,
            })
          : formatNotificationEventLabel(event.type)
        : event.type === "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE"
          ? detailT("titles.appUpdateAvailable", {
              version: readPayloadValue(event, "latest_version") || "",
            }).trim()
          : formatNotificationEventLabel(event.type);

  return matchedCount > 1 ? `${baseTitle} x${matchedCount}` : baseTitle;
};

const toSeverity = (event: SystemEventEnvelope): NotificationSeverity => {
  switch (event.level) {
    case "INFO":
      return "info";
    case "WARN":
      return "warn";
    case "ERROR":
      return "error";
    case "CRITICAL":
      return "critical";
    default:
      return "info";
  }
};

export const buildNotificationMessage = (args: {
  event: SystemEventEnvelope;
  rule: NotificationRule;
  matchedCount: number;
  groupKey: string;
  locale?: string | null;
}): NotificationMessage => {
  return withNotificationLocale(args.locale, () => {
    const details = buildNotificationDetails(args);

    return normalizeNotificationMessage(
      {
        title: buildNotificationTitle(args.event, args.matchedCount),
        summary: details.summary,
        body_text: details.body_text,
        body_markdown: details.body_markdown,
        severity: toSeverity(args.event),
        facts: details.facts,
        actions: [],
        mentions: [],
        dedupe_key: `${args.rule.id}:${args.groupKey}`,
        occurred_at: args.event.happened_at,
        event_id: args.event.id,
        metadata: {
          event_type: args.event.type,
          event_level: args.event.level,
          event_source: args.event.source,
          rule_id: args.rule.id,
          rule_name: args.rule.name,
          group_key: args.groupKey,
          matched_count: args.matchedCount,
          window_seconds: args.rule.window_seconds,
          threshold_count: args.rule.threshold_count,
          locale: activeNotificationLocale,
        },
      },
      activeNotificationLocale,
    );
  });
};
