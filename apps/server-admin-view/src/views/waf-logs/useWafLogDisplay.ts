import { computed, type ComputedRef, type Ref } from "vue";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import type { WAFEvent, WAFInterruptionInfo, WAFRuleMatch } from "@/types";

const WAF_RULE_PATH_PREFIX = "/usr/local/apps/@appdata/fn-knock/waf";

type TranslateParams = Record<string, unknown>;

const ruleFileBasename = (value?: string) => {
  const normalized = String(value || "")
    .trim()
    .replace(/\\/g, "/");
  return normalized.split("/").pop() || "";
};

const isCRSBlockingEvaluationRule = (rule: WAFRuleMatch) => {
  const filename = ruleFileBasename(rule.file).toLowerCase();
  return (
    filename === "request-949-blocking-evaluation.conf" ||
    filename === "response-959-blocking-evaluation.conf" ||
    rule.tags?.some((tag) => tag.toLowerCase() === "anomaly-evaluation")
  );
};

const hasRuleDescription = (rule: WAFRuleMatch) =>
  Boolean(rule.message?.trim() || rule.data?.trim());

const detailFields = [
  { key: "time", labelKey: "admin.wafLogs.detailFields.time" },
  { key: "trace_id", label: "Trace ID" },
  { key: "transaction_id", labelKey: "admin.wafLogs.detailFields.transactionId" },
  { key: "action", labelKey: "admin.wafLogs.detailFields.action" },
  { key: "mode", labelKey: "admin.wafLogs.detailFields.mode" },
  { key: "status", labelKey: "admin.wafLogs.detailFields.status" },
  { key: "client_ip", labelKey: "admin.wafLogs.detailFields.clientIp" },
  { key: "ipLocation", labelKey: "admin.wafLogs.detailFields.ipLocation" },
  { key: "remote_addr", labelKey: "admin.wafLogs.detailFields.remoteAddr" },
  { key: "method", labelKey: "admin.wafLogs.detailFields.method" },
  { key: "scheme", labelKey: "admin.wafLogs.detailFields.scheme" },
  { key: "host", label: "Host" },
  { key: "path", labelKey: "admin.wafLogs.detailFields.path" },
  { key: "query", label: "Query" },
  { key: "request_uri", labelKey: "admin.wafLogs.detailFields.requestUri" },
  { key: "user_agent", label: "User-Agent" },
  { key: "referer", label: "Referer" },
  { key: "route_type", labelKey: "admin.wafLogs.detailFields.routeType" },
  { key: "route_key", labelKey: "admin.wafLogs.detailFields.routeKey" },
  { key: "upstream", labelKey: "admin.wafLogs.detailFields.upstream" },
  { key: "bundle_id", labelKey: "admin.wafLogs.detailFields.bundleId" },
  { key: "bundle_hash", label: "Bundle Hash" },
  { key: "rule_ids", labelKey: "admin.wafLogs.detailFields.ruleIds" },
  { key: "rules", labelKey: "admin.wafLogs.detailFields.rules" },
  { key: "interruption", labelKey: "admin.wafLogs.detailFields.interruption" },
  { key: "error", labelKey: "admin.wafLogs.detailFields.error" },
] as const;

export const useWafLogDisplay = ({
  activeEvent,
  activeEventWithIpLocation,
  locale,
  translate,
}: {
  activeEvent: Ref<WAFEvent | null>;
  activeEventWithIpLocation: ComputedRef<(WAFEvent & { ipLocation: string }) | null>;
  locale: Ref<string>;
  translate: (key: string, params?: TranslateParams) => string;
}) => {
  const actionLabel = (value?: string) => {
    switch (value) {
      case "block":
      case "deny":
        return translate("admin.wafLogs.actions.block");
      case "log":
      case "detect":
        return translate("admin.wafLogs.actions.record");
      case "pass":
        return translate("admin.wafLogs.actions.pass");
      default:
        return value || "-";
    }
  };

  const actionVariant = (value?: string) => {
    if (value === "block" || value === "deny") return "destructive";
    if (value === "log" || value === "detect") return "secondary";
    return "outline";
  };

  const modeLabel = (value?: string) => {
    switch (value) {
      case "detection":
        return translate("admin.wafLogs.modes.detection");
      case "blocking":
        return translate("admin.wafLogs.modes.blocking");
      case "off":
        return translate("admin.wafLogs.modes.off");
      default:
        return value || "-";
    }
  };

  const routeTypeLabel = (value?: string) => {
    switch (value) {
      case "path_rule":
        return translate("admin.wafLogs.routeTypes.pathRule");
      case "host_rule":
        return translate("admin.wafLogs.routeTypes.hostRule");
      case "auth_proxy":
        return translate("admin.wafLogs.routeTypes.authProxy");
      case "select":
        return translate("admin.wafLogs.routeTypes.select");
      case "preflight":
        return translate("admin.wafLogs.routeTypes.preflight");
      case "slash_redirect":
        return translate("admin.wafLogs.routeTypes.slashRedirect");
      case "favicon":
        return translate("admin.wafLogs.routeTypes.favicon");
      case "general_blacklist":
        return translate("admin.wafLogs.routeTypes.generalBlacklist");
      case "not_found":
        return translate("admin.wafLogs.routeTypes.notFound");
      default:
        return value || "-";
    }
  };

  const formatDate = (value?: string) =>
    formatDateTimeSafe(value, { locale: locale.value });

  const formatRuleIds = (value?: number[]) =>
    value && value.length > 0 ? value.map((id) => `#${id}`).join(", ") : "-";

  const getPrimaryRule = (event: WAFEvent): WAFRuleMatch | undefined => {
    const rules = event.rules || [];
    const interruptedRuleId = event.interruption?.rule_id;
    const contributingRules = rules.filter(
      (rule) => !isCRSBlockingEvaluationRule(rule),
    );
    if (interruptedRuleId) {
      const interruptedRule = rules.find(
        (rule) => rule.id === interruptedRuleId,
      );
      if (interruptedRule && !isCRSBlockingEvaluationRule(interruptedRule)) {
        return interruptedRule;
      }
    }
    return (
      contributingRules.find(hasRuleDescription) ||
      contributingRules.find((rule) => rule.disruptive) ||
      contributingRules[0] ||
      rules.find(hasRuleDescription) ||
      rules.find((rule) => rule.disruptive) ||
      rules[0]
    );
  };

  const formatPrimaryRuleId = (event: WAFEvent) => {
    const primaryRule = getPrimaryRule(event);
    if (!primaryRule) return formatRuleIds(event.rule_ids);
    const ruleIds = new Set([
      ...(event.rule_ids || []),
      ...(event.rules || []).map((rule) => rule.id),
    ]);
    const otherCount = Math.max(0, ruleIds.size - 1);
    return otherCount > 0
      ? translate("admin.wafLogs.moreRules", {
          id: primaryRule.id,
          count: otherCount,
        })
      : `#${primaryRule.id}`;
  };

  const formatRuleSummary = (event: WAFEvent) => {
    const firstRule = getPrimaryRule(event);
    if (!firstRule) return "";
    return firstRule.message || firstRule.data || "";
  };

  const formatRuleFilePath = (value?: string) => {
    const normalized = String(value || "")
      .trim()
      .replace(/\\/g, "/");
    if (!normalized) return "";

    const lowerPath = normalized.toLowerCase();
    const lowerPrefix = WAF_RULE_PATH_PREFIX.toLowerCase();
    if (lowerPath === lowerPrefix) return "";
    if (lowerPath.startsWith(`${lowerPrefix}/`)) {
      return normalized.slice(WAF_RULE_PATH_PREFIX.length + 1);
    }

    return normalized;
  };

  const formatRuleFileLocation = (rule: WAFRuleMatch) => {
    const file = formatRuleFilePath(rule.file);
    if (!file) return "";
    return rule.line ? `${file}:${rule.line}` : file;
  };

  const formatRuleLocationSummary = (event: WAFEvent) => {
    const firstRule = getPrimaryRule(event);
    if (!firstRule?.file) return "";
    return formatRuleFileLocation(firstRule);
  };

  const formatRules = (
    value: WAFRuleMatch[] | undefined,
    event?: WAFEvent | null,
  ) => {
    if (!value || value.length === 0) return "-";
    const primaryRuleId = event ? getPrimaryRule(event)?.id : undefined;
    return [...value]
      .sort((left, right) => {
        if (primaryRuleId) {
          if (left.id === primaryRuleId) return -1;
          if (right.id === primaryRuleId) return 1;
        }
        return 0;
      })
      .map((rule) => {
        const parts = [`#${rule.id}`];
        if (rule.phase) parts.push(`phase ${rule.phase}`);
        if (rule.severity) parts.push(rule.severity);
        const location = formatRuleFileLocation(rule);
        if (location) parts.push(location);
        if (rule.message) parts.push(rule.message);
        return parts.join(" · ");
      })
      .join("\n");
  };

  const formatInterruption = (value: WAFInterruptionInfo | undefined) => {
    if (!value) return "-";
    return [
      value.rule_id ? `rule ${value.rule_id}` : "",
      value.action || "",
      value.status ? `HTTP ${value.status}` : "",
    ]
      .filter(Boolean)
      .join(" · ");
  };

  const localizedDetailFields = computed(() =>
    detailFields.map((field) => ({
      key: field.key,
      label: "label" in field ? field.label : translate(field.labelKey),
    })),
  );

  const detailItems = computed(() =>
    buildDetailFields(
      activeEventWithIpLocation.value,
      localizedDetailFields.value,
      {
        format: (key, value) => {
          if (key === "time") return formatDate(value as string);
          if (key === "action") return actionLabel(String(value || ""));
          if (key === "mode") return modeLabel(String(value || ""));
          if (key === "route_type") return routeTypeLabel(String(value || ""));
          if (key === "rule_ids") return formatRuleIds(value as number[]);
          if (key === "rules")
            return formatRules(value as WAFRuleMatch[], activeEvent.value);
          if (key === "interruption")
            return formatInterruption(value as WAFInterruptionInfo);
          if (value === undefined || value === null || value === "") return "-";
          return value;
        },
      },
    ),
  );

  const detailCopyText = computed(() =>
    detailItems.value
      .map((item) => `${item.label}: ${String(item.value)}`)
      .join("\n"),
  );

  return {
    actionLabel,
    actionVariant,
    detailCopyText,
    detailItems,
    formatPrimaryRuleId,
    formatRuleLocationSummary,
    formatRuleSummary,
    modeLabel,
    routeTypeLabel,
  };
};
