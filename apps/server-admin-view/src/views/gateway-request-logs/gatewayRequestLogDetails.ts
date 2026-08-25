import type { GatewayLogEntry } from "@/types";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import type { GatewayLogTranslator } from "./gateway-request-log-types";
import {
  accessModeLabel,
  authDecisionLabel,
  authGrantStateLabel,
  credentialMethodLabel,
  formatBoolean,
  formatDuration,
  formatRuleIds,
  routeTypeLabel,
  wafActionLabel,
  wafModeLabel,
} from "./gatewayRequestLogPresentation";

const detailFields = [
  { key: "time", labelKey: "admin.gatewayRequestLogs.detailFields.time" },
  { key: "method", labelKey: "admin.gatewayRequestLogs.detailFields.method" },
  { key: "scheme", labelKey: "admin.gatewayRequestLogs.detailFields.scheme" },
  { key: "host", labelKey: "admin.gatewayRequestLogs.detailFields.host" },
  { key: "path", labelKey: "admin.gatewayRequestLogs.detailFields.path" },
  { key: "query", labelKey: "admin.gatewayRequestLogs.detailFields.query" },
  {
    key: "request_uri",
    labelKey: "admin.gatewayRequestLogs.detailFields.requestUri",
  },
  {
    key: "protocol",
    labelKey: "admin.gatewayRequestLogs.detailFields.protocol",
  },
  { key: "status", labelKey: "admin.gatewayRequestLogs.detailFields.status" },
  {
    key: "duration_ms",
    labelKey: "admin.gatewayRequestLogs.detailFields.duration",
  },
  {
    key: "client_ip",
    labelKey: "admin.gatewayRequestLogs.detailFields.clientIp",
  },
  {
    key: "ipLocation",
    labelKey: "admin.gatewayRequestLogs.detailFields.ipLocation",
  },
  {
    key: "remote_ip",
    labelKey: "admin.gatewayRequestLogs.detailFields.remoteIp",
  },
  {
    key: "remote_addr",
    labelKey: "admin.gatewayRequestLogs.detailFields.remoteAddr",
  },
  {
    key: "user_agent",
    labelKey: "admin.gatewayRequestLogs.detailFields.userAgent",
  },
  {
    key: "referer",
    labelKey: "admin.gatewayRequestLogs.detailFields.referer",
  },
  {
    key: "logged_in",
    labelKey: "admin.gatewayRequestLogs.detailFields.loggedIn",
  },
  {
    key: "auth_required",
    labelKey: "admin.gatewayRequestLogs.detailFields.authRequired",
  },
  {
    key: "auth_decision",
    labelKey: "admin.gatewayRequestLogs.detailFields.authDecision",
  },
  {
    key: "auth_rule_group_id",
    labelKey: "admin.gatewayRequestLogs.detailFields.authRuleGroupId",
  },
  {
    key: "auth_grant_state",
    labelKey: "admin.gatewayRequestLogs.detailFields.authGrantState",
  },
  {
    key: "auth_credential_method",
    labelKey: "admin.gatewayRequestLogs.detailFields.authCredentialMethod",
  },
  {
    key: "auth_credential_name",
    labelKey: "admin.gatewayRequestLogs.detailFields.authCredentialName",
  },
  {
    key: "auth_credential_id",
    labelKey: "admin.gatewayRequestLogs.detailFields.authCredentialId",
  },
  {
    key: "auth_linked_totp_name",
    labelKey: "admin.gatewayRequestLogs.detailFields.authLinkedTotpName",
  },
  {
    key: "auth_linked_totp_id",
    labelKey: "admin.gatewayRequestLogs.detailFields.authLinkedTotpId",
  },
  {
    key: "access_mode",
    labelKey: "admin.gatewayRequestLogs.detailFields.accessMode",
  },
  {
    key: "route_type",
    labelKey: "admin.gatewayRequestLogs.detailFields.routeType",
  },
  {
    key: "route_key",
    labelKey: "admin.gatewayRequestLogs.detailFields.routeKey",
  },
  {
    key: "upstream",
    labelKey: "admin.gatewayRequestLogs.detailFields.upstream",
  },
  {
    key: "upstream_error_class",
    labelKey: "admin.gatewayRequestLogs.detailFields.upstreamErrorClass",
  },
  { key: "matched", labelKey: "admin.gatewayRequestLogs.detailFields.matched" },
  {
    key: "bytes_in",
    labelKey: "admin.gatewayRequestLogs.detailFields.bytesIn",
  },
  {
    key: "bytes_out",
    labelKey: "admin.gatewayRequestLogs.detailFields.bytesOut",
  },
  { key: "tls", labelKey: "admin.gatewayRequestLogs.detailFields.tls" },
  {
    key: "websocket",
    labelKey: "admin.gatewayRequestLogs.detailFields.websocket",
  },
  {
    key: "eo_connecting_ip",
    labelKey: "admin.gatewayRequestLogs.detailFields.eoConnectingIp",
  },
  {
    key: "ali_real_client_ip",
    labelKey: "admin.gatewayRequestLogs.detailFields.aliRealClientIp",
  },
  {
    key: "x_forwarded_for",
    labelKey: "admin.gatewayRequestLogs.detailFields.xForwardedFor",
  },
  {
    key: "x_real_ip",
    labelKey: "admin.gatewayRequestLogs.detailFields.xRealIp",
  },
  {
    key: "waf_blocked",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafBlocked",
  },
  {
    key: "general_blacklist_blocked",
    labelKey: "admin.gatewayRequestLogs.detailFields.generalBlacklistBlocked",
  },
  {
    key: "waf_trace_id",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafTraceId",
  },
  {
    key: "waf_mode",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafMode",
  },
  {
    key: "waf_action",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafAction",
  },
  {
    key: "waf_rule_ids",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafRuleIds",
  },
  {
    key: "waf_bundle",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafBundle",
  },
] as const;

const localizeDetailFields = (t: GatewayLogTranslator) =>
  detailFields.map((field) => ({
    key: field.key,
    label: t(field.labelKey),
  }));

export const buildGatewayLogDetailItems = (
  entry: GatewayLogEntry | null | undefined,
  t: GatewayLogTranslator,
  locale: string,
) =>
  buildDetailFields(entry, localizeDetailFields(t), {
    format: (key, value) => {
      if (key === "time") return formatDateTimeSafe(value, { locale });
      if (key === "duration_ms") return formatDuration(value);
      if (
        key === "logged_in" ||
        key === "auth_required" ||
        key === "matched" ||
        key === "tls" ||
        key === "websocket" ||
        key === "waf_blocked" ||
        key === "general_blacklist_blocked"
      ) {
        return formatBoolean(Boolean(value), t);
      }
      if (key === "route_type") return routeTypeLabel(String(value || ""), t);
      if (key === "auth_decision")
        return authDecisionLabel(String(value || ""), t);
      if (key === "auth_grant_state")
        return authGrantStateLabel(String(value || ""), t);
      if (key === "auth_credential_method")
        return credentialMethodLabel(String(value || ""), t) || "-";
      if (key === "access_mode") return accessModeLabel(String(value || ""), t);
      if (key === "waf_action") return wafActionLabel(String(value || ""), t);
      if (key === "waf_mode") return wafModeLabel(String(value || ""), t);
      if (key === "waf_rule_ids") return formatRuleIds(value as number[]);
      if (value === undefined || value === null || value === "") return "-";
      return value;
    },
  });

export const buildGatewayLogDetailCopyText = (
  detailItems: ReturnType<typeof buildGatewayLogDetailItems>,
) =>
  detailItems.map((item) => `${item.label}: ${String(item.value)}`).join("\n");
