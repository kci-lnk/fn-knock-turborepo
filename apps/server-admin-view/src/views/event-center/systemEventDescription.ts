import type { SystemEventLevel, SystemEventRecord } from "../../types";
import type {
  SystemEventTranslate,
  SystemEventValueFormatters,
} from "./systemEventValueFormatters";

export type EventOriginDisplay = {
  key: string;
  ip: string;
  location?: string;
};

export const systemEventTypeTextClass = (event: SystemEventRecord) =>
  event.level === "INFO" ? "text-black" : "text-red-700";

export const systemEventLevelBadgeClass = (level: SystemEventLevel) => {
  switch (level) {
    case "INFO":
      return "border-emerald-500/25 bg-emerald-500/10 text-emerald-700";
    case "WARN":
      return "border-amber-500/25 bg-amber-500/10 text-amber-700";
    case "ERROR":
      return "border-rose-500/25 bg-rose-500/10 text-rose-700";
    case "CRITICAL":
      return "border-fuchsia-500/25 bg-fuchsia-500/10 text-fuchsia-700";
    default:
      return "";
  }
};

export const resolveSystemEventOrigins = (
  event: SystemEventRecord,
): EventOriginDisplay[] => {
  const payload = event.payload ?? {};
  const origins: EventOriginDisplay[] = [];
  const pushOrigin = (ipKey: string, locationKey: string) => {
    const ip = String(payload[ipKey] ?? "").trim();
    if (!ip) return;
    const location = String(payload[locationKey] ?? "").trim();
    origins.push({
      key: `${ipKey}:${ip}`,
      ip,
      ...(location ? { location } : {}),
    });
  };
  if (event.type === "FN_EVENT_AUTH_SESSION_IP_DRIFT") {
    pushOrigin("to_ip", "to_ip_location");
    if (origins.length === 0) pushOrigin("from_ip", "from_ip_location");
  } else {
    pushOrigin("ip", "ip_location");
  }
  return origins;
};

export const describeSystemEvent = (
  event: SystemEventRecord,
  translate: SystemEventTranslate,
  formatters: SystemEventValueFormatters,
) => {
  const payload = event.payload ?? {};
  const {
    formatAuthMethodLabel,
    formatCredentialDisplay,
    formatIpDisplay,
    formatLogoutSourceLabel,
    formatSessionCommentInline,
    formatTunnelLabel,
    formatTunnelStatusLabel,
    formatWAFOutcomeLabel,
    shortId,
  } = formatters;

  switch (event.type) {
    case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
      const authMethod = String(payload.auth_method || "");
      const authProviderName = String(payload.auth_provider_name || "").trim();
      const authMethodLabel =
        (authMethod === "OIDC" || authMethod === "LDAP") && authProviderName
          ? translate("admin.eventCenter.events.viaProvider", {
              provider: authProviderName,
            })
          : translate("admin.eventCenter.events.viaMethod", {
              method:
                formatAuthMethodLabel(authMethod) ||
                String(payload.auth_method || "-"),
            });
      return translate("admin.eventCenter.events.authLoginSuccess", {
        credential: formatCredentialDisplay(
          payload.credential_name,
          payload.linked_totp_name,
          payload.auth_method,
        ),
        method: authMethodLabel,
        ip: formatIpDisplay(payload.ip),
        comment: formatSessionCommentInline(payload.session_comment),
      });
    }
    case "FN_EVENT_AUTH_LOGOUT":
      return translate("admin.eventCenter.events.authLogout", {
        credential: formatCredentialDisplay(
          payload.credential_name,
          payload.linked_totp_name,
          payload.auth_method,
        ),
        source:
          formatLogoutSourceLabel(payload.logout_source) ||
          String(payload.logout_source || "-"),
        ip: formatIpDisplay(payload.ip),
        comment: formatSessionCommentInline(payload.session_comment),
      });
    case "FN_EVENT_AUTH_LOGIN_FAILURE": {
      const attempts = String(payload.attempts || "-");
      const retryAfterSeconds = Number(payload.retry_after_seconds);
      const isExternalFailure = ["OIDC", "LDAP"].includes(
        String(payload.method ?? "").trim(),
      );
      const credentialName = String(
        (isExternalFailure && payload.auth_provider_name) ||
          payload.credential_name ||
          "",
      ).trim();
      const hasCredentialContext =
        (!!credentialName && !credentialName.startsWith("!")) ||
        payload.linked_totp_name !== undefined;
      const credentialContext = hasCredentialContext
        ? formatCredentialDisplay(
            credentialName,
            payload.linked_totp_name,
            payload.method,
          )
        : "";
      const retry =
        Number.isFinite(retryAfterSeconds) && retryAfterSeconds > 0
          ? translate("admin.eventCenter.events.retryAfter", {
              seconds: retryAfterSeconds,
            })
          : "";
      return hasCredentialContext
        ? translate("admin.eventCenter.events.authFailureWithCredential", {
            credential: credentialContext,
            ip: formatIpDisplay(payload.ip),
            attempts,
            retry,
          })
        : translate("admin.eventCenter.events.authFailureWithoutCredential", {
            ip: formatIpDisplay(payload.ip),
            attempts,
            retry,
          });
    }
    case "FN_EVENT_AUTH_SESSION_IP_DRIFT": {
      const credentialName = String(payload.credential_name ?? "").trim();
      const linkedTotpName = String(payload.linked_totp_name ?? "").trim();
      const hasCredentialContext = Boolean(credentialName || linkedTotpName);
      const sessionLabel = hasCredentialContext
        ? `${formatCredentialDisplay(
            payload.credential_name,
            payload.linked_totp_name,
            payload.auth_method,
          )} ${translate("admin.eventCenter.events.session")}`
        : `${translate("admin.eventCenter.events.session")} ${shortId(
            String(payload.session_id || ""),
            14,
          )}`;
      return translate("admin.eventCenter.events.sessionIpDrift", {
        session: sessionLabel,
        fromIp: String(formatIpDisplay(payload.from_ip)),
        toIp: String(formatIpDisplay(payload.to_ip)),
        comment: formatSessionCommentInline(payload.session_comment),
      });
    }
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED":
      return translate("admin.eventCenter.events.scannerBlocked", {
        ip: formatIpDisplay(payload.ip),
        count: String(payload.hit_count || "-"),
      });
    case "FN_EVENT_DDNS_UPDATE_COMPLETED":
      return translate("admin.eventCenter.events.ddnsUpdated", {
        provider: String(payload.provider || "-"),
        result: payload.success
          ? translate("admin.eventCenter.events.success")
          : translate("admin.eventCenter.events.failure"),
        message: String(payload.message || "-"),
      });
    case "FN_EVENT_WOL_WAKE_COMPLETED":
      return translate("admin.eventCenter.events.wolWakeCompleted", {
        target: String(payload.target_name || payload.target_id || "-"),
        relay: String(payload.relay_name || payload.relay_id || "-"),
        result: payload.success
          ? translate("admin.eventCenter.events.success")
          : String(
              payload.status || translate("admin.eventCenter.events.failure"),
            ),
        latency: String(payload.latency_ms ?? "-"),
      });
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED":
      return translate("admin.eventCenter.events.gatewayThrottleBlocked", {
        ip: formatIpDisplay(payload.ip),
        seconds: String(payload.block_seconds || "-"),
      });
    case "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED":
      return translate("admin.eventCenter.events.gatewayVisibilityBlocked", {
        ip: formatIpDisplay(payload.ip),
        host: String(payload.host || "-"),
        path: String(payload.path || "-"),
      });
    case "FN_EVENT_WAF_BLOCKED":
      return translate("admin.eventCenter.events.wafBlocked", {
        ip: formatIpDisplay(payload.ip),
        outcome: formatWAFOutcomeLabel(payload.action, payload.mode),
        rules: payload.rule_ids
          ? translate("admin.eventCenter.events.wafRuleSuffix", {
              rules: String(payload.rule_ids),
            })
          : "",
      });
    case "FN_EVENT_SSH_LOGIN_SUCCESS":
      return translate("admin.eventCenter.events.sshLoginSuccess", {
        username: String(payload.username || "-"),
        ip: formatIpDisplay(payload.ip),
      });
    case "FN_EVENT_SSH_LOGIN_FAILURE":
      return translate("admin.eventCenter.events.sshLoginFailure", {
        username: String(payload.username || "-"),
        ip: formatIpDisplay(payload.ip),
        attempts: String(payload.attempts || "-"),
      });
    case "FN_EVENT_SSH_IP_BLOCKED":
      return translate("admin.eventCenter.events.sshBlocked", {
        ip: formatIpDisplay(payload.ip),
        reason:
          String(payload.reason) === "cidr_not_allowed"
            ? translate("admin.eventCenter.events.sshReasonCidr")
            : translate("admin.eventCenter.events.sshReasonThreshold"),
      });
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE":
      return translate("admin.eventCenter.events.appUpdateAvailable", {
        latest: String(payload.latest_version || "-"),
        current: String(payload.local_version || "-"),
        suffix: payload.force_update
          ? translate("admin.eventCenter.events.updateSoonSuffix")
          : "",
      });
    case "FN_EVENT_SYSTEM_CPU_ALERT":
      return translate("admin.eventCenter.events.cpuAlert", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
      return translate("admin.eventCenter.events.cpuRecovered", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
      return translate("admin.eventCenter.events.memoryAlert", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED":
      return translate("admin.eventCenter.events.memoryRecovered", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_TUNNEL_FRP_CONNECTED":
    case "FN_EVENT_TUNNEL_FRP_DISCONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_CONNECTED":
    case "FN_EVENT_TUNNEL_CLOUDFLARED_DISCONNECTED": {
      const tunnel =
        formatTunnelLabel(payload.tunnel) ||
        (event.type.includes("CLOUDFLARED") ? "Cloudflared" : "FRP");
      const status =
        formatTunnelStatusLabel(payload.status) ||
        (event.type.endsWith("_CONNECTED")
          ? formatTunnelStatusLabel("connected")
          : formatTunnelStatusLabel("disconnected"));
      const message = String(payload.message || "").trim();
      return translate("admin.eventCenter.events.tunnelStatusDescription", {
        tunnel,
        status,
        message: message
          ? translate("admin.eventCenter.events.messageSuffix", { message })
          : "",
      });
    }
    case "FN_EVENT_RUNTIME_STARTED":
    case "FN_EVENT_RUNTIME_STOPPED":
    case "FN_EVENT_RUNTIME_RESTARTED":
    case "FN_EVENT_RUNTIME_HEALTH_FAILED":
    case "FN_EVENT_RUNTIME_RECOVERED":
    case "FN_EVENT_RUNTIME_ABNORMAL_EXIT":
      return translate("admin.eventCenter.events.runtimeStatusDescription", {
        component: String(payload.component || event.subject?.id || "-"),
        event: translate(`admin.eventCenter.eventTypes.${event.type}`),
        reason: String(payload.reason_code || "-"),
      });
    default:
      return JSON.stringify(payload);
  }
};
