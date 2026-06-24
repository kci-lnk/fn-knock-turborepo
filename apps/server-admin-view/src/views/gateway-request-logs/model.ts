import type { GatewayLogEntry } from "../../types";
import { normalizeIpKey } from "../../composables/useIpLocationBatch";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";

type GatewayLogTranslator = (
  key: string,
  params?: Record<string, unknown>,
) => string;

export const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const LIMIT_OPTIONS = ["10", "20", "50", "100"] as const;

export const STATUS_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.statusFilters.all" },
  {
    value: "2xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.success2xx",
  },
  {
    value: "3xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.redirect3xx",
  },
  {
    value: "4xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.client4xx",
  },
  {
    value: "5xx",
    labelKey: "admin.gatewayRequestLogs.statusFilters.server5xx",
  },
  {
    value: "401",
    labelKey: "admin.gatewayRequestLogs.statusFilters.unauthorized401",
  },
  {
    value: "403",
    labelKey: "admin.gatewayRequestLogs.statusFilters.forbidden403",
  },
  {
    value: "404",
    labelKey: "admin.gatewayRequestLogs.statusFilters.notFound404",
  },
  {
    value: "500",
    labelKey: "admin.gatewayRequestLogs.statusFilters.serverError500",
  },
  {
    value: "502",
    labelKey: "admin.gatewayRequestLogs.statusFilters.badGateway502",
  },
  {
    value: "503",
    labelKey: "admin.gatewayRequestLogs.statusFilters.unavailable503",
  },
] as const;

export const LOGIN_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.loginFilters.all" },
  { value: "true", labelKey: "admin.gatewayRequestLogs.loginFilters.loggedIn" },
  {
    value: "false",
    labelKey: "admin.gatewayRequestLogs.loginFilters.notLoggedIn",
  },
] as const;

export const WAF_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.wafFilters.all" },
  { value: "has_waf", labelKey: "admin.gatewayRequestLogs.wafFilters.hasWaf" },
  { value: "none", labelKey: "admin.gatewayRequestLogs.wafFilters.none" },
] as const;

export const UNRECORDED_CREDENTIAL_FILTER = "__unrecorded__";

export type GatewayStatusFilterValue =
  (typeof STATUS_FILTER_OPTIONS)[number]["value"];
export type GatewayLoginFilterValue =
  (typeof LOGIN_FILTER_OPTIONS)[number]["value"];
export type GatewayWAFFilterValue =
  (typeof WAF_FILTER_OPTIONS)[number]["value"];

export const getGatewayLogOptionLabel = <
  TOption extends { value: string; labelKey: string },
>(
  options: readonly TOption[],
  value: string,
  fallbackLabelKey: string,
  t: GatewayLogTranslator,
) =>
  t(options.find((item) => item.value === value)?.labelKey || fallbackLabelKey);

export const formatRuleIds = (value?: number[]) =>
  value && value.length > 0 ? value.join(", ") : "-";

export const hasWAFSignal = (entry: GatewayLogEntry) =>
  Boolean(entry.waf_trace_id) ||
  Boolean(entry.waf_bundle) ||
  Boolean(entry.waf_action) ||
  entry.waf_blocked === true ||
  (Array.isArray(entry.waf_rule_ids) && entry.waf_rule_ids.length > 0);

export const getWAFAction = (entry: GatewayLogEntry) =>
  String(entry.waf_action || "").toLowerCase();

export const isWAFBlocked = (entry: GatewayLogEntry) =>
  entry.waf_blocked === true ||
  getWAFAction(entry) === "block" ||
  getWAFAction(entry) === "deny";

export const getWAFBadgeClass = (entry: GatewayLogEntry) => {
  if (isWAFBlocked(entry)) {
    return "border-red-500/20 bg-transparent text-red-600/80 hover:bg-red-500/[0.04] dark:text-red-300/80";
  }
  if (getWAFAction(entry) === "pass") {
    return "border-emerald-500/20 bg-transparent text-emerald-600/80 hover:bg-emerald-500/[0.04] dark:text-emerald-300/80";
  }
  return "border-muted-foreground/20 bg-transparent text-muted-foreground hover:bg-muted/30";
};

export const getStatusTextClass = (status: number) => {
  if (status >= 500) return "text-red-600";
  if (status >= 400) return "text-amber-600";
  return "text-foreground";
};

export const getStatusDotClass = (status: number) => {
  if (status >= 500) return "bg-red-500";
  if (status >= 400) return "bg-amber-500";
  return "bg-muted-foreground/35";
};

export const getEntryClientIp = (entry: GatewayLogEntry) =>
  entry.client_ip || entry.remote_ip || "";

export const getEntryActionIp = (entry: GatewayLogEntry) => {
  const clientIp = getEntryClientIp(entry);
  return normalizeIpKey(clientIp) || clientIp.trim();
};

export const buildGatewayLogSelectionKey = (
  entry: GatewayLogEntry,
  index: number,
  cursor: string | null | undefined,
) =>
  [
    cursor || "first",
    index,
    entry.time || "",
    entry.method || "",
    entry.host || "",
    entry.request_uri || entry.path || "",
    entry.status ?? "",
    entry.duration_ms ?? "",
    getEntryActionIp(entry),
    entry.remote_addr || entry.remote_ip || "",
    entry.waf_trace_id || "",
  ].join("|");

export const getForwardedHeaderLines = (entry: GatewayLogEntry) => {
  const lines: string[] = [];

  if (entry.eo_connecting_ip) {
    lines.push(`EO-Connecting-IP: ${entry.eo_connecting_ip}`);
  }
  if (entry.ali_real_client_ip) {
    lines.push(`Ali-Real-Client-IP: ${entry.ali_real_client_ip}`);
  }
  if (entry.x_forwarded_for) {
    lines.push(`X-Forwarded-For: ${entry.x_forwarded_for}`);
  }
  if (entry.x_real_ip) {
    lines.push(`X-Real-IP: ${entry.x_real_ip}`);
  }

  return lines;
};

export const wafActionLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (value) {
    case "block":
    case "deny":
      return t("admin.wafLogs.actions.block");
    case "log":
    case "detect":
      return t("admin.wafLogs.actions.record");
    case "pass":
      return t("admin.wafLogs.actions.pass");
    default:
      return value || "-";
  }
};

export const wafModeLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (value) {
    case "detection":
      return t("admin.wafLogs.modes.detection");
    case "blocking":
      return t("admin.wafLogs.modes.blocking");
    case "off":
      return t("admin.wafLogs.modes.off");
    default:
      return value || "-";
  }
};

export const wafBadgeLabel = (
  entry: GatewayLogEntry,
  t: GatewayLogTranslator,
) => {
  if (isWAFBlocked(entry))
    return t("admin.gatewayRequestLogs.wafBadges.blocked");
  const action = getWAFAction(entry);
  if (action === "pass") return t("admin.gatewayRequestLogs.wafBadges.pass");
  if (action === "log" || action === "detect")
    return t("admin.gatewayRequestLogs.wafBadges.record");
  return t("admin.gatewayRequestLogs.wafBadges.hit");
};

export const wafBadgeMeta = (
  entry: GatewayLogEntry,
  t: GatewayLogTranslator,
) => {
  if (entry.waf_rule_ids?.length) {
    return entry.waf_rule_ids.map((id) => `#${id}`).join(" ");
  }
  return entry.waf_trace_id || wafActionLabel(entry.waf_action, t);
};

export const wafBadgeTitle = (
  entry: GatewayLogEntry,
  t: GatewayLogTranslator,
) => {
  const parts = [wafBadgeLabel(entry, t)];
  if (entry.waf_trace_id) parts.push(`Trace: ${entry.waf_trace_id}`);
  if (entry.waf_rule_ids?.length) {
    parts.push(
      t("admin.gatewayRequestLogs.wafBadges.rules", {
        rules: entry.waf_rule_ids.join(", "),
      }),
    );
  }
  if (entry.waf_bundle)
    parts.push(
      t("admin.gatewayRequestLogs.wafBadges.bundle", {
        bundle: entry.waf_bundle,
      }),
    );
  return parts.join(" · ");
};

export const routeTypeLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (value) {
    case "path_rule":
      return t("admin.wafLogs.routeTypes.pathRule");
    case "host_rule":
      return t("admin.wafLogs.routeTypes.hostRule");
    case "auth_proxy":
      return t("admin.wafLogs.routeTypes.authProxy");
    case "select":
      return t("admin.wafLogs.routeTypes.select");
    case "preflight":
      return t("admin.wafLogs.routeTypes.preflight");
    case "slash_redirect":
      return t("admin.wafLogs.routeTypes.slashRedirect");
    case "favicon":
      return t("admin.wafLogs.routeTypes.favicon");
    case "general_blacklist":
      return t("admin.wafLogs.routeTypes.generalBlacklist");
    case "not_found":
      return t("admin.wafLogs.routeTypes.notFound");
    default:
      return value || "-";
  }
};

export const authDecisionLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (value) {
    case "passed":
      return t("admin.gatewayRequestLogs.authDecisions.passed");
    case "redirected":
      return t("admin.gatewayRequestLogs.authDecisions.redirected");
    case "denied":
      return t("admin.gatewayRequestLogs.authDecisions.denied");
    case "access_denied":
      return t("admin.gatewayRequestLogs.authDecisions.accessDenied");
    case "root_mode_redirect":
      return t("admin.gatewayRequestLogs.authDecisions.rootModeRedirect");
    case "not_required":
      return t("admin.gatewayRequestLogs.authDecisions.notRequired");
    case "proxy":
      return t("admin.gatewayRequestLogs.authDecisions.proxy");
    case "error":
      return t("admin.gatewayRequestLogs.authDecisions.error");
    case "general_blacklist_blocked":
      return t(
        "admin.gatewayRequestLogs.authDecisions.generalBlacklistBlocked",
      );
    default:
      return value || "-";
  }
};

export const credentialMethodLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (String(value || "").toUpperCase()) {
    case "TOTP":
      return t("admin.gatewayRequestLogs.credentialMethods.totp");
    case "PASSKEY":
      return t("admin.gatewayRequestLogs.credentialMethods.passkey");
    case "OIDC":
      return t("admin.gatewayRequestLogs.credentialMethods.oidc");
    default:
      return value || "";
  }
};

export const formatAuthCredential = (
  entry: GatewayLogEntry,
  t: GatewayLogTranslator,
) => {
  const method = credentialMethodLabel(entry.auth_credential_method, t);
  const name = entry.auth_credential_name || entry.auth_credential_id || "";
  const primary = method && name ? `${method} / ${name}` : method || name || "";
  if (!primary) return "";

  const linkedTotpName =
    String(entry.auth_credential_method || "").toUpperCase() === "TOTP"
      ? ""
      : entry.auth_linked_totp_name || entry.auth_linked_totp_id || "";
  if (!linkedTotpName) return primary;

  return `${primary} (${t("admin.gatewayRequestLogs.linkedTotp", {
    name: linkedTotpName,
  })})`;
};

export const formatDuration = (value?: number) => {
  if (!Number.isFinite(value)) return "-";
  return `${value} ms`;
};

export const formatBoolean = (
  value: boolean | undefined,
  t: GatewayLogTranslator,
) => {
  return value
    ? t("admin.gatewayRequestLogs.boolean.yes")
    : t("admin.gatewayRequestLogs.boolean.no");
};

const detailFields = [
  { key: "time", labelKey: "admin.gatewayRequestLogs.detailFields.time" },
  { key: "method", labelKey: "admin.gatewayRequestLogs.detailFields.method" },
  { key: "scheme", labelKey: "admin.gatewayRequestLogs.detailFields.scheme" },
  { key: "host", label: "Host" },
  { key: "path", labelKey: "admin.gatewayRequestLogs.detailFields.path" },
  { key: "query", label: "Query" },
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
  { key: "user_agent", label: "User-Agent" },
  { key: "referer", label: "Referer" },
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
  { key: "matched", labelKey: "admin.gatewayRequestLogs.detailFields.matched" },
  {
    key: "bytes_in",
    labelKey: "admin.gatewayRequestLogs.detailFields.bytesIn",
  },
  {
    key: "bytes_out",
    labelKey: "admin.gatewayRequestLogs.detailFields.bytesOut",
  },
  { key: "tls", label: "TLS" },
  { key: "websocket", label: "WebSocket" },
  { key: "eo_connecting_ip", label: "EO-Connecting-IP" },
  { key: "ali_real_client_ip", label: "Ali-Real-Client-IP" },
  { key: "x_forwarded_for", label: "X-Forwarded-For" },
  { key: "x_real_ip", label: "X-Real-IP" },
  {
    key: "waf_blocked",
    labelKey: "admin.gatewayRequestLogs.detailFields.wafBlocked",
  },
  {
    key: "general_blacklist_blocked",
    labelKey: "admin.gatewayRequestLogs.detailFields.generalBlacklistBlocked",
  },
  { key: "waf_trace_id", label: "WAF Trace ID" },
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
    label: "label" in field ? field.label : t(field.labelKey),
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
      if (key === "auth_credential_method")
        return credentialMethodLabel(String(value || ""), t) || "-";
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
