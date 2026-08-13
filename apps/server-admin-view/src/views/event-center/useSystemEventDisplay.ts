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
import { detailFieldDefinitions } from "./systemEventDetailFields";
import {
  describeSystemEvent,
  resolveSystemEventOrigins,
  systemEventLevelBadgeClass,
  systemEventTypeTextClass,
  type EventOriginDisplay,
} from "./systemEventDescription";
import {
  createSystemEventValueFormatters,
  type SystemEventTranslate,
} from "./systemEventValueFormatters";

export type { EventOriginDisplay };

export const useSystemEventDisplay = ({
  activeEvent,
  translate,
}: {
  activeEvent: Ref<SystemEventRecord | null>;
  translate: SystemEventTranslate;
}) => {
  const formatSystemEventTypeLabel = (type: SystemEventType) =>
    translate(`admin.eventCenter.eventTypes.${type}`);
  const formatSystemEventLevelLabel = (level: SystemEventLevel) =>
    translate(`admin.eventCenter.levels.${level}`);
  const formatSystemEventSourceLabel = (source: SystemEventSource) =>
    translate(`admin.eventCenter.sources.${source}`);
  const formatDate = (value: string) => formatDateTimeSafe(value);
  const formatters = createSystemEventValueFormatters(translate);
  const {
    formatAuthMethodLabel,
    formatBoolean,
    formatCheckReasonLabel,
    formatDriftSourceLabel,
    formatGrantTypeLabel,
    formatIpDisplay,
    formatIpSourceLabel,
    formatLogoutSourceLabel,
    formatPercentage,
    formatPostLoginGrantModeLabel,
    formatSubject,
    formatTunnelLabel,
    formatTunnelStatusLabel,
    formatUpdateScopeLabel,
    formatWafActionLabel,
    formatWafModeLabel,
  } = formatters;

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
            [
              "happened_at",
              "expires_at",
              "login_time",
              "blocked_until",
              "blocked_at",
            ].includes(key)
          ) {
            return formatDate(String(value || ""));
          }
          if (key === "subject") return formatSubject(event.subject, false);
          if (key === "logout_source")
            return formatLogoutSourceLabel(value) || String(value);
          if (
            key === "method" &&
            event.type === "FN_EVENT_GATEWAY_VISIBILITY_BLOCKED"
          ) {
            return String(value);
          }
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
          if (key === "visibility_scope" || key === "visibility_mode") {
            const normalized = String(value || "");
            const category = key === "visibility_scope" ? "Scope" : "Mode";
            return normalized
              ? translate(
                  `admin.eventCenter.events.visibility${category}.${normalized}`,
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
            ["usage_percent", "threshold_percent", "recover_percent"].includes(
              key,
            )
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

  return {
    describeEvent: (event: SystemEventRecord) =>
      describeSystemEvent(event, translate, formatters),
    detailCopyText,
    detailItems,
    eventTypeTextClass: systemEventTypeTextClass,
    formatIpDisplay,
    formatSystemEventLevelLabel,
    formatSystemEventSourceLabel,
    formatSystemEventTypeLabel,
    levelBadgeClass: systemEventLevelBadgeClass,
    resolveEventOrigins: resolveSystemEventOrigins,
  };
};
