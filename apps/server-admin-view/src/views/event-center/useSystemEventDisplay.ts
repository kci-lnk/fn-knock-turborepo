import { computed, type Ref } from "vue";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import { routeTypeLabel } from "@/lib/routeType";
import type {
  SystemEventLevel,
  SystemEventRecord,
  SystemEventSource,
  SystemEventType,
} from "../../types";

type TranslateParams = Record<string, unknown>;
type Translate = (key: string, params?: TranslateParams) => string;

const detailFieldDefinitions = [
  { key: "id", labelKey: "admin.eventCenter.events.detailFields.id" },
  { key: "type", labelKey: "admin.eventCenter.events.detailFields.type" },
  { key: "level", labelKey: "admin.eventCenter.events.detailFields.level" },
  { key: "source", labelKey: "admin.eventCenter.events.detailFields.source" },
  {
    key: "happened_at",
    labelKey: "admin.eventCenter.events.detailFields.happened_at",
  },
  {
    key: "dedupe_key",
    labelKey: "admin.eventCenter.events.detailFields.dedupe_key",
  },
  { key: "subject", labelKey: "admin.eventCenter.events.detailFields.subject" },
  {
    key: "credential_name",
    labelKey: "admin.eventCenter.events.detailFields.credential_name",
  },
  {
    key: "linked_totp_name",
    labelKey: "admin.eventCenter.events.detailFields.linked_totp_name",
  },
  {
    key: "session_comment",
    labelKey: "admin.eventCenter.events.detailFields.session_comment",
  },
  {
    key: "credential_id",
    labelKey: "admin.eventCenter.events.detailFields.credential_id",
  },
  {
    key: "auth_method",
    labelKey: "admin.eventCenter.events.detailFields.auth_method",
  },
  {
    key: "auth_provider_name",
    labelKey: "admin.eventCenter.events.detailFields.auth_provider_name",
  },
  {
    key: "grant_type",
    labelKey: "admin.eventCenter.events.detailFields.grant_type",
  },
  {
    key: "post_login_ip_grant_mode",
    labelKey: "admin.eventCenter.events.detailFields.post_login_ip_grant_mode",
  },
  {
    key: "remember_me",
    labelKey: "admin.eventCenter.events.detailFields.remember_me",
  },
  {
    key: "session_id",
    labelKey: "admin.eventCenter.events.detailFields.session_id",
  },
  { key: "ip", labelKey: "admin.eventCenter.events.detailFields.ip" },
  {
    key: "ip_location",
    labelKey: "admin.eventCenter.events.detailFields.ip_location",
  },
  {
    key: "user_agent",
    labelKey: "admin.eventCenter.events.detailFields.user_agent",
  },
  {
    key: "expires_at",
    labelKey: "admin.eventCenter.events.detailFields.expires_at",
  },
  {
    key: "login_time",
    labelKey: "admin.eventCenter.events.detailFields.login_time",
  },
  {
    key: "logout_source",
    labelKey: "admin.eventCenter.events.detailFields.logout_source",
  },
  {
    key: "attempts",
    labelKey: "admin.eventCenter.events.detailFields.attempts",
  },
  {
    key: "threshold",
    labelKey: "admin.eventCenter.events.detailFields.threshold",
  },
  {
    key: "retry_after_seconds",
    labelKey: "admin.eventCenter.events.detailFields.retry_after_seconds",
  },
  {
    key: "blocked_until",
    labelKey: "admin.eventCenter.events.detailFields.blocked_until",
  },
  { key: "method", labelKey: "admin.eventCenter.events.detailFields.method" },
  { key: "scheme", labelKey: "admin.eventCenter.events.detailFields.scheme" },
  {
    key: "visibility_scope",
    labelKey: "admin.eventCenter.events.detailFields.visibility_scope",
  },
  {
    key: "visibility_mode",
    labelKey: "admin.eventCenter.events.detailFields.visibility_mode",
  },
  {
    key: "drift_source",
    labelKey: "admin.eventCenter.events.detailFields.drift_source",
  },
  { key: "from_ip", labelKey: "admin.eventCenter.events.detailFields.from_ip" },
  {
    key: "from_ip_location",
    labelKey: "admin.eventCenter.events.detailFields.from_ip_location",
  },
  { key: "to_ip", labelKey: "admin.eventCenter.events.detailFields.to_ip" },
  {
    key: "to_ip_location",
    labelKey: "admin.eventCenter.events.detailFields.to_ip_location",
  },
  {
    key: "blocked_at",
    labelKey: "admin.eventCenter.events.detailFields.blocked_at",
  },
  {
    key: "window_minutes",
    labelKey: "admin.eventCenter.events.detailFields.window_minutes",
  },
  {
    key: "hit_count",
    labelKey: "admin.eventCenter.events.detailFields.hit_count",
  },
  {
    key: "provider",
    labelKey: "admin.eventCenter.events.detailFields.provider",
  },
  { key: "success", labelKey: "admin.eventCenter.events.detailFields.success" },
  { key: "message", labelKey: "admin.eventCenter.events.detailFields.message" },
  {
    key: "update_scope",
    labelKey: "admin.eventCenter.events.detailFields.update_scope",
  },
  {
    key: "ip_source",
    labelKey: "admin.eventCenter.events.detailFields.ip_source",
  },
  {
    key: "local_version",
    labelKey: "admin.eventCenter.events.detailFields.local_version",
  },
  {
    key: "latest_version",
    labelKey: "admin.eventCenter.events.detailFields.latest_version",
  },
  {
    key: "force_update",
    labelKey: "admin.eventCenter.events.detailFields.force_update",
  },
  {
    key: "release_notes",
    labelKey: "admin.eventCenter.events.detailFields.release_notes",
  },
  {
    key: "check_reason",
    labelKey: "admin.eventCenter.events.detailFields.check_reason",
  },
  { key: "tunnel", labelKey: "admin.eventCenter.events.detailFields.tunnel" },
  { key: "status", labelKey: "admin.eventCenter.events.detailFields.status" },
  { key: "pid", labelKey: "admin.eventCenter.events.detailFields.pid" },
  {
    key: "component",
    labelKey: "admin.eventCenter.events.detailFields.component",
  },
  {
    key: "incident_id",
    labelKey: "admin.eventCenter.events.detailFields.incident_id",
  },
  {
    key: "instance_id",
    labelKey: "admin.eventCenter.events.detailFields.instance_id",
  },
  {
    key: "reason_code",
    labelKey: "admin.eventCenter.events.detailFields.reason_code",
  },
  {
    key: "duration_ms",
    labelKey: "admin.eventCenter.events.detailFields.duration_ms",
  },
  {
    key: "process_state",
    labelKey: "admin.eventCenter.events.detailFields.process_state",
  },
  {
    key: "previous_ipv4",
    labelKey: "admin.eventCenter.events.detailFields.previous_ipv4",
  },
  {
    key: "next_ipv4",
    labelKey: "admin.eventCenter.events.detailFields.next_ipv4",
  },
  {
    key: "previous_ipv6",
    labelKey: "admin.eventCenter.events.detailFields.previous_ipv6",
  },
  {
    key: "next_ipv6",
    labelKey: "admin.eventCenter.events.detailFields.next_ipv6",
  },
  {
    key: "block_seconds",
    labelKey: "admin.eventCenter.events.detailFields.block_seconds",
  },
  {
    key: "requests_per_second",
    labelKey: "admin.eventCenter.events.detailFields.requests_per_second",
  },
  { key: "burst", labelKey: "admin.eventCenter.events.detailFields.burst" },
  {
    key: "trace_id",
    labelKey: "admin.eventCenter.events.detailFields.trace_id",
  },
  { key: "mode", labelKey: "admin.eventCenter.events.detailFields.mode" },
  { key: "action", labelKey: "admin.eventCenter.events.detailFields.action" },
  {
    key: "request_uri",
    labelKey: "admin.eventCenter.events.detailFields.request_uri",
  },
  {
    key: "bundle_id",
    labelKey: "admin.eventCenter.events.detailFields.bundle_id",
  },
  {
    key: "rule_ids",
    labelKey: "admin.eventCenter.events.detailFields.rule_ids",
  },
  {
    key: "route_type",
    labelKey: "admin.eventCenter.events.detailFields.route_type",
  },
  {
    key: "route_key",
    labelKey: "admin.eventCenter.events.detailFields.route_key",
  },
  { key: "host", labelKey: "admin.eventCenter.events.detailFields.host" },
  { key: "path", labelKey: "admin.eventCenter.events.detailFields.path" },
  {
    key: "is_auth_route",
    labelKey: "admin.eventCenter.events.detailFields.is_auth_route",
  },
  {
    key: "hostname",
    labelKey: "admin.eventCenter.events.detailFields.hostname",
  },
  {
    key: "usage_percent",
    labelKey: "admin.eventCenter.events.detailFields.usage_percent",
  },
  {
    key: "threshold_percent",
    labelKey: "admin.eventCenter.events.detailFields.threshold_percent",
  },
  {
    key: "recover_percent",
    labelKey: "admin.eventCenter.events.detailFields.recover_percent",
  },
  {
    key: "sample_interval_seconds",
    labelKey: "admin.eventCenter.events.detailFields.sample_interval_seconds",
  },
  {
    key: "sustain_seconds",
    labelKey: "admin.eventCenter.events.detailFields.sustain_seconds",
  },
] as const;

const DRIFT_SOURCE_LABEL_KEYS: Record<string, string> = {
  "proxy-session": "admin.eventCenter.events.driftSource.proxySession",
  "fnos-token": "admin.eventCenter.events.driftSource.fnosToken",
  "session-refresh": "admin.eventCenter.events.driftSource.sessionRefresh",
  "browser-session": "admin.eventCenter.events.driftSource.browserSession",
};

const CHECK_REASON_LABEL_KEYS: Record<string, string> = {
  cron: "admin.eventCenter.events.checkReason.cron",
  manual: "admin.eventCenter.events.checkReason.manual",
  "manual-check-and-download":
    "admin.eventCenter.events.checkReason.manualCheckAndDownload",
  "download-bootstrap":
    "admin.eventCenter.events.checkReason.downloadBootstrap",
};

const AUTO_IP_GRANT_COMMENT_VALUES = new Set([
  "server.auth.autoIpGrantComment",
  "登录后自动授权",
  "登入後自動授權",
  "Automatically authorized after sign-in",
]);

export type EventOriginDisplay = {
  key: string;
  ip: string;
  location?: string;
};

export const useSystemEventDisplay = ({
  activeEvent,
  translate,
}: {
  activeEvent: Ref<SystemEventRecord | null>;
  translate: Translate;
}) => {
  const formatSystemEventTypeLabel = (type: SystemEventType) =>
    translate(`admin.eventCenter.eventTypes.${type}`);

  const formatSystemEventLevelLabel = (level: SystemEventLevel) =>
    translate(`admin.eventCenter.levels.${level}`);

  const formatSystemEventSourceLabel = (source: SystemEventSource) =>
    translate(`admin.eventCenter.sources.${source}`);

  const formatDate = (value: string) => formatDateTimeSafe(value);

  const localizedDetailFieldDefinitions = computed(() =>
    detailFieldDefinitions.map((field) => ({
      key: field.key,
      label: translate(
        field.key === "method" &&
          activeEvent.value?.type === "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"
          ? "admin.eventCenter.events.detailFields.request_method"
          : field.labelKey,
      ),
    })),
  );

  const eventTypeTextClass = (event: SystemEventRecord) =>
    event.level === "INFO" ? "text-black" : "text-red-700";

  const translateValue = (
    prefix: string,
    value: unknown,
    keyMap: Record<string, string> = {},
  ) => {
    const key = String(value ?? "");
    if (!key) return "";
    const messageKey = keyMap[key] || `${prefix}.${key}`;
    const translated = translate(messageKey);
    return translated === messageKey ? key : translated;
  };

  const formatSubjectKindLabel = (
    kind: NonNullable<SystemEventRecord["subject"]>["kind"],
  ) => translateValue("admin.eventCenter.events.subjectKind", kind);

  const formatLogoutSourceLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.logoutSource", value);

  const formatAuthMethodLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.authMethod", value);

  const formatDriftSourceLabel = (value: unknown) =>
    translateValue(
      "admin.eventCenter.events.driftSource",
      value,
      DRIFT_SOURCE_LABEL_KEYS,
    );

  const formatGrantTypeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.grantType", value);

  const formatPostLoginGrantModeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.postLoginGrantMode", value);

  const formatUpdateScopeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.updateScope", value);

  const formatIpSourceLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.ipSource", value);

  const formatCheckReasonLabel = (value: unknown) =>
    translateValue(
      "admin.eventCenter.events.checkReason",
      value,
      CHECK_REASON_LABEL_KEYS,
    );

  const formatTunnelLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.tunnel", value);

  const formatTunnelStatusLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.tunnelStatus", value);

  const formatWafModeLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.wafMode", value);

  const formatWafActionLabel = (value: unknown) =>
    translateValue("admin.eventCenter.events.wafAction", value);

  const shortId = (value: string, size = 10) =>
    value.length <= size
      ? value
      : `${value.slice(0, Math.max(4, size - 5))}...${value.slice(-4)}`;

  const formatSubject = (
    subject: SystemEventRecord["subject"] | undefined,
    shortenId = false,
  ) => {
    if (!subject) return "-";
    const kind = formatSubjectKindLabel(subject.kind) || subject.kind;
    const id = shortenId ? shortId(subject.id, 18) : subject.id;
    return `${kind} · ${id}`;
  };

  const shortenMiddle = (value: string, leading = 12, trailing = 10) =>
    value.length <= leading + trailing + 3
      ? value
      : `${value.slice(0, leading)}...${value.slice(-trailing)}`;

  const formatIpDisplay = (value: unknown) => {
    const ip = String(value ?? "").trim();

    if (!ip) return "-";
    if (ip.includes(":") && ip.length > 24) {
      return shortenMiddle(ip, 14, 11);
    }
    if (ip.length > 24) {
      return shortenMiddle(ip, 12, 8);
    }
    return ip;
  };

  const formatPercentage = (value: unknown) =>
    value === undefined || value === null || value === ""
      ? "-"
      : `${String(value)}%`;

  const formatBoolean = (value: unknown) =>
    value === undefined || value === null
      ? "-"
      : value
        ? translate("admin.eventCenter.events.yes")
        : translate("admin.eventCenter.events.no");

  const formatCredentialDisplay = (
    credentialName: unknown,
    linkedTotpName: unknown,
    authMethod: unknown,
  ) => {
    const credential =
      String(credentialName ?? "").trim() ||
      translate("admin.eventCenter.events.unknownCredential");
    const linkedTotp = String(linkedTotpName ?? "").trim();

    if (String(authMethod ?? "") === "PASSKEY" && linkedTotp) {
      return `Passkey「${credential}」 / TOTP「${linkedTotp}」`;
    }

    return credential;
  };

  const formatSessionCommentInline = (value: unknown) => {
    const rawComment = String(value ?? "").trim();
    const comment = AUTO_IP_GRANT_COMMENT_VALUES.has(rawComment)
      ? translate("auth.autoIpGrantComment")
      : rawComment;
    return comment
      ? translate("admin.eventCenter.events.sessionComment", { comment })
      : "";
  };

  const isWAFBlockingAction = (action: unknown, mode: unknown) => {
    const normalizedAction = String(action ?? "").toLowerCase();
    if (normalizedAction === "block" || normalizedAction === "deny")
      return true;
    if (
      normalizedAction === "detect" ||
      normalizedAction === "log" ||
      normalizedAction === "pass"
    ) {
      return false;
    }
    return String(mode ?? "").toLowerCase() === "blocking";
  };

  const formatWAFOutcomeLabel = (action: unknown, mode: unknown) => {
    if (isWAFBlockingAction(action, mode)) {
      return formatWafActionLabel("block");
    }
    return formatWafActionLabel(action) || formatWafActionLabel("log");
  };

  const detailItems = computed(() => {
    const event = activeEvent.value;
    if (!event) return [];

    const payload = event.payload ?? {};
    const detailRecord: Record<string, unknown> = {
      id: event.id,
      type: event.type,
      level: event.level,
      source: event.source,
      happened_at: event.happened_at,
      dedupe_key: event.dedupe_key,
      subject: event.subject,
      ...payload,
    };

    return buildDetailFields(
      detailRecord,
      localizedDetailFieldDefinitions.value,
      {
        format: (key, value) => {
          if (key === "type")
            return formatSystemEventTypeLabel(value as SystemEventType);
          if (key === "level")
            return formatSystemEventLevelLabel(value as SystemEventLevel);
          if (key === "source")
            return formatSystemEventSourceLabel(value as SystemEventSource);
          if (
            key === "happened_at" ||
            key === "expires_at" ||
            key === "login_time" ||
            key === "blocked_until" ||
            key === "blocked_at"
          ) {
            return formatDate(String(value || ""));
          }
          if (key === "subject") return formatSubject(event.subject, false);
          if (key === "logout_source")
            return formatLogoutSourceLabel(value) || String(value);
          if (
            key === "method" &&
            event.type === "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"
          )
            return String(value);
          if (key === "auth_method" || key === "method")
            return formatAuthMethodLabel(value) || String(value);
          if (key === "drift_source")
            return formatDriftSourceLabel(value) || String(value);
          if (key === "grant_type")
            return formatGrantTypeLabel(value) || String(value);
          if (key === "post_login_ip_grant_mode")
            return formatPostLoginGrantModeLabel(value) || String(value);
          if (key === "update_scope")
            return formatUpdateScopeLabel(value) || String(value);
          if (key === "ip_source")
            return formatIpSourceLabel(value) || String(value);
          if (key === "check_reason")
            return formatCheckReasonLabel(value) || String(value);
          if (key === "tunnel")
            return formatTunnelLabel(value) || String(value);
          if (key === "mode") return formatWafModeLabel(value) || String(value);
          if (key === "action")
            return formatWafActionLabel(value) || String(value);
          if (key === "rule_ids" && Array.isArray(value))
            return value.join(", ");
          if (key === "route_type")
            return routeTypeLabel(String(value || ""), translate);
          if (key === "visibility_scope") {
            const normalized = String(value || "");
            return normalized
              ? translate(
                  `admin.eventCenter.events.visibilityScope.${normalized}`,
                )
              : "-";
          }
          if (key === "visibility_mode") {
            const normalized = String(value || "");
            return normalized
              ? translate(
                  `admin.eventCenter.events.visibilityMode.${normalized}`,
                )
              : "-";
          }
          if (key === "status")
            return formatTunnelStatusLabel(value) || String(value);
          if (key === "remember_me" || key === "is_auth_route")
            return formatBoolean(value);
          if (key === "force_update")
            return formatBoolean(value === true || value === "true");
          if (key === "success")
            return value === undefined || value === null
              ? "-"
              : value
                ? translate("admin.eventCenter.events.success")
                : translate("admin.eventCenter.events.failure");
          if (
            key === "usage_percent" ||
            key === "threshold_percent" ||
            key === "recover_percent"
          ) {
            return formatPercentage(value);
          }
          if (value === undefined || value === null || value === "") return "-";
          if (Array.isArray(value)) return value.join(", ");
          return String(value);
        },
      },
    );
  });

  const detailCopyText = computed(() => {
    const lines = detailItems.value.map(
      (item) => `${item.label}: ${String(item.value)}`,
    );
    const tags = activeEvent.value?.tags || [];

    if (tags.length > 0) {
      lines.push(
        "",
        `${translate("admin.eventCenter.events.tagsCopyLabel")}：${tags.join(", ")}`,
      );
    }

    return lines.join("\n");
  });

  const levelBadgeClass = (level: SystemEventLevel) => {
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

  const resolveEventOrigins = (
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

    switch (event.type) {
      case "FN_EVENT_AUTH_SESSION_IP_DRIFT":
        pushOrigin("to_ip", "to_ip_location");
        if (origins.length === 0) {
          pushOrigin("from_ip", "from_ip_location");
        }
        break;
      default:
        pushOrigin("ip", "ip_location");
        break;
    }

    return origins;
  };

  const describeEvent = (event: SystemEventRecord) => {
    const payload = event.payload ?? {};

    switch (event.type) {
      case "FN_EVENT_AUTH_LOGIN_SUCCESS": {
        const authMethod = String(payload.auth_method || "");
        const authProviderName = String(
          payload.auth_provider_name || "",
        ).trim();
        const authMethodLabel =
          authMethod === "OIDC" && authProviderName
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
        const isOidcFailure = String(payload.method ?? "").trim() === "OIDC";
        const credentialName = String(
          (isOidcFailure && payload.auth_provider_name) ||
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
          : `${translate("admin.eventCenter.events.session")} ${shortId(String(payload.session_id || ""), 14)}`;
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
      case "FN_EVENT_WAF_BLOCKED": {
        const outcomeLabel = formatWAFOutcomeLabel(
          payload.action,
          payload.mode,
        );
        return translate("admin.eventCenter.events.wafBlocked", {
          ip: formatIpDisplay(payload.ip),
          outcome: outcomeLabel,
          rules: payload.rule_ids
            ? translate("admin.eventCenter.events.wafRuleSuffix", {
                rules: String(payload.rule_ids),
              })
            : "",
        });
      }
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

  return {
    describeEvent,
    detailCopyText,
    detailItems,
    eventTypeTextClass,
    formatIpDisplay,
    formatSystemEventLevelLabel,
    formatSystemEventSourceLabel,
    formatSystemEventTypeLabel,
    levelBadgeClass,
    resolveEventOrigins,
  };
};
