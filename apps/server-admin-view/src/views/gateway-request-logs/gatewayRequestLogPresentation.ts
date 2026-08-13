import type { GatewayLogEntry } from "@/types";
import { AUTH_DECISION_LABEL_KEYS } from "@/lib/gatewayLogLabels";
import { routeTypeLabel as resolveRouteTypeLabel } from "@/lib/routeType";
import { normalizeIpKey } from "@/composables/useIpLocationBatch";
import type { GatewayLogTranslator } from "./gateway-request-log-types";

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
  if (entry.waf_bundle) {
    parts.push(
      t("admin.gatewayRequestLogs.wafBadges.bundle", {
        bundle: entry.waf_bundle,
      }),
    );
  }
  return parts.join(" · ");
};

export const routeTypeLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => resolveRouteTypeLabel(value, t);

export const authDecisionLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  const normalized = value?.trim().toLowerCase() || "";
  const translationKey = AUTH_DECISION_LABEL_KEYS[normalized];
  return translationKey ? t(translationKey) : value || "-";
};

export const authGrantStateLabel = (
  value: string | undefined,
  t: GatewayLogTranslator,
) => {
  switch (value) {
    case "issued":
      return t("admin.gatewayRequestLogs.grantStates.issued");
    case "renewed":
      return t("admin.gatewayRequestLogs.grantStates.renewed");
    case "reused":
      return t("admin.gatewayRequestLogs.grantStates.reused");
    case "transient":
      return t("admin.gatewayRequestLogs.grantStates.transient");
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
    case "LDAP":
      return t("admin.gatewayRequestLogs.credentialMethods.ldap");
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
) =>
  value
    ? t("admin.gatewayRequestLogs.boolean.yes")
    : t("admin.gatewayRequestLogs.boolean.no");
