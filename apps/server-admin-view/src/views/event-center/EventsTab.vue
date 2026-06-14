<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, Loader2, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "../../lib/api";
import type {
  SystemEventLevel,
  SystemEventRecord,
  SystemEventSource,
  SystemEventType,
} from "../../types";
import {
  SYSTEM_EVENT_LEVEL_FILTER_OPTIONS as LEVEL_OPTIONS,
  SYSTEM_EVENT_SOURCE_FILTER_OPTIONS as SOURCE_OPTIONS,
  SYSTEM_EVENT_TYPE_FILTER_OPTIONS as TYPE_OPTIONS,
} from "./constants";

const { t } = useI18n();

const formatSystemEventTypeLabel = (type: SystemEventType) =>
  t(`admin.eventCenter.eventTypes.${type}`);

const formatSystemEventLevelLabel = (level: SystemEventLevel) =>
  t(`admin.eventCenter.levels.${level}`);

const formatSystemEventSourceLabel = (source: SystemEventSource) =>
  t(`admin.eventCenter.sources.${source}`);

const formatOptionLabel = (option: { labelKey: string }) => t(option.labelKey);

const selectedType = ref<SystemEventType | "all">("all");
const selectedLevel = ref<SystemEventLevel | "all">("all");
const selectedSource = ref<SystemEventSource | "all">("all");
const isDetailsOpen = ref(false);
const activeEvent = ref<SystemEventRecord | null>(null);

const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.eventCenter.events.deleteFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.eventCenter.events.deleteEventFailed"),
      ),
    });
  },
});

const {
  items: events,
  total: totalEvents,
  loading,
  searchQuery,
  currentPage,
  limit,
  parsedLimit,
  selectedKeys,
  isAllSelected,
  fetchList: fetchEvents,
  handleSearch,
  handlePageChange,
  handleLimitChange,
  toggleSelect,
  clearSelection,
} = usePagedSelectionList<SystemEventRecord, string>({
  fetchPage: async ({ page, limit, query }) => {
    const result = await EventCenterAPI.getEvents({
      page,
      limit,
      search: query,
      type: selectedType.value,
      level: selectedLevel.value,
      source: selectedSource.value,
    });

    if (!(result.success || result.data)) {
      throw new Error(
        result.message || t("admin.eventCenter.events.loadFailed"),
      );
    }

    return {
      items: result.data.events || [],
      total: result.data.total || 0,
    };
  },
  getKey: (event) => event.id,
  onError: (error) => {
    toast.error(t("admin.eventCenter.events.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.eventCenter.events.eventListLoadFailed"),
      ),
    });
  },
});

const showTableSkeleton = useDelayedLoading(
  () => loading.value && events.value.length === 0,
);
const hasSelectedEvents = computed(() => selectedKeys.value.size > 0);

const viewDetails = (event: SystemEventRecord) => {
  activeEvent.value = event;
  isDetailsOpen.value = true;
};

const formatDate = (value: string) => formatDateTimeSafe(value);

const deleteEvents = async (ids: string[]) => {
  await runDelete(() => EventCenterAPI.deleteEvents(ids), {
    onSuccess: async (result) => {
      if (result.success || result.message === "success") {
        toast.success(t("admin.eventCenter.events.deleteSuccess"));
        clearSelection();
        await fetchEvents();
        return;
      }
      toast.error(t("admin.eventCenter.events.deleteFailed"), {
        description:
          result.message || t("admin.eventCenter.events.deleteEventFailed"),
      });
    },
  });
};

const handleFilterChange = () => {
  currentPage.value = 1;
  fetchEvents();
};

watch([selectedType, selectedLevel, selectedSource], handleFilterChange);

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

const localizedDetailFieldDefinitions = computed(() =>
  detailFieldDefinitions.map((field) => ({
    key: field.key,
    label: t(field.labelKey),
  })),
);

const eventTypeTextClass = (event: SystemEventRecord) =>
  event.level === "INFO" ? "text-black" : "text-red-700";

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

const translateValue = (
  prefix: string,
  value: unknown,
  keyMap: Record<string, string> = {},
) => {
  const key = String(value ?? "");
  if (!key) return "";
  const messageKey = keyMap[key] || `${prefix}.${key}`;
  const translated = t(messageKey);
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
      ? t("admin.eventCenter.events.yes")
      : t("admin.eventCenter.events.no");

const formatCredentialDisplay = (
  credentialName: unknown,
  linkedTotpName: unknown,
  authMethod: unknown,
) => {
  const credential =
    String(credentialName ?? "").trim() ||
    t("admin.eventCenter.events.unknownCredential");
  const linkedTotp = String(linkedTotpName ?? "").trim();

  if (String(authMethod ?? "") === "PASSKEY" && linkedTotp) {
    return `Passkey「${credential}」 / TOTP「${linkedTotp}」`;
  }

  return credential;
};

const AUTO_IP_GRANT_COMMENT_VALUES = new Set([
  "server.auth.autoIpGrantComment",
  "登录后自动授权",
  "登入後自動授權",
  "Automatically authorized after sign-in",
]);

const formatSessionCommentInline = (value: unknown) => {
  const rawComment = String(value ?? "").trim();
  const comment = AUTO_IP_GRANT_COMMENT_VALUES.has(rawComment)
    ? t("auth.autoIpGrantComment")
    : rawComment;
  return comment
    ? t("admin.eventCenter.events.sessionComment", { comment })
    : "";
};

const isWAFBlockingAction = (action: unknown, mode: unknown) => {
  const normalizedAction = String(action ?? "").toLowerCase();
  if (normalizedAction === "block" || normalizedAction === "deny") return true;
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
        if (key === "tunnel") return formatTunnelLabel(value) || String(value);
        if (key === "mode") return formatWafModeLabel(value) || String(value);
        if (key === "action")
          return formatWafActionLabel(value) || String(value);
        if (key === "rule_ids" && Array.isArray(value)) return value.join(", ");
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
              ? t("admin.eventCenter.events.success")
              : t("admin.eventCenter.events.failure");
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
      `${t("admin.eventCenter.events.tagsCopyLabel")}：${tags.join(", ")}`,
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

const shortId = (value: string, size = 10) =>
  value.length <= size
    ? value
    : `${value.slice(0, Math.max(4, size - 5))}...${value.slice(-4)}`;

type EventOriginDisplay = {
  key: string;
  ip: string;
  location?: string;
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
      const authProviderName = String(payload.auth_provider_name || "").trim();
      const authMethodLabel =
        authMethod === "OIDC" && authProviderName
          ? t("admin.eventCenter.events.viaProvider", {
              provider: authProviderName,
            })
          : t("admin.eventCenter.events.viaMethod", {
              method:
                formatAuthMethodLabel(authMethod) ||
                String(payload.auth_method || "-"),
            });
      return t("admin.eventCenter.events.authLoginSuccess", {
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
      return t("admin.eventCenter.events.authLogout", {
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
      const credentialName = String(payload.credential_name ?? "").trim();
      const hasCredentialContext =
        (!!credentialName && !credentialName.startsWith("!")) ||
        payload.linked_totp_name !== undefined;
      const credentialContext = hasCredentialContext
        ? formatCredentialDisplay(
            payload.credential_name,
            payload.linked_totp_name,
            payload.method,
          )
        : "";
      const retry =
        Number.isFinite(retryAfterSeconds) && retryAfterSeconds > 0
          ? t("admin.eventCenter.events.retryAfter", {
              seconds: retryAfterSeconds,
            })
          : "";
      return hasCredentialContext
        ? t("admin.eventCenter.events.authFailureWithCredential", {
            credential: credentialContext,
            ip: formatIpDisplay(payload.ip),
            attempts,
            retry,
          })
        : t("admin.eventCenter.events.authFailureWithoutCredential", {
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
          )} ${t("admin.eventCenter.events.session")}`
        : `${t("admin.eventCenter.events.session")} ${shortId(String(payload.session_id || ""), 14)}`;
      return t("admin.eventCenter.events.sessionIpDrift", {
        session: sessionLabel,
        fromIp: String(formatIpDisplay(payload.from_ip)),
        toIp: String(formatIpDisplay(payload.to_ip)),
        comment: formatSessionCommentInline(payload.session_comment),
      });
    }
    case "FN_EVENT_SECURITY_SCANNER_BLOCKED":
      return t("admin.eventCenter.events.scannerBlocked", {
        ip: formatIpDisplay(payload.ip),
        count: String(payload.hit_count || "-"),
      });
    case "FN_EVENT_DDNS_UPDATE_COMPLETED":
      return t("admin.eventCenter.events.ddnsUpdated", {
        provider: String(payload.provider || "-"),
        result: Boolean(payload.success)
          ? t("admin.eventCenter.events.success")
          : t("admin.eventCenter.events.failure"),
        message: String(payload.message || "-"),
      });
    case "FN_EVENT_GATEWAY_THROTTLE_BLOCKED":
      return t("admin.eventCenter.events.gatewayThrottleBlocked", {
        ip: formatIpDisplay(payload.ip),
        seconds: String(payload.block_seconds || "-"),
      });
    case "FN_EVENT_WAF_BLOCKED": {
      const outcomeLabel = formatWAFOutcomeLabel(payload.action, payload.mode);
      return t("admin.eventCenter.events.wafBlocked", {
        ip: formatIpDisplay(payload.ip),
        outcome: outcomeLabel,
        rules: payload.rule_ids
          ? t("admin.eventCenter.events.wafRuleSuffix", {
              rules: String(payload.rule_ids),
            })
          : "",
      });
    }
    case "FN_EVENT_SSH_LOGIN_SUCCESS":
      return t("admin.eventCenter.events.sshLoginSuccess", {
        username: String(payload.username || "-"),
        ip: formatIpDisplay(payload.ip),
      });
    case "FN_EVENT_SSH_LOGIN_FAILURE":
      return t("admin.eventCenter.events.sshLoginFailure", {
        username: String(payload.username || "-"),
        ip: formatIpDisplay(payload.ip),
        attempts: String(payload.attempts || "-"),
      });
    case "FN_EVENT_SSH_IP_BLOCKED":
      return t("admin.eventCenter.events.sshBlocked", {
        ip: formatIpDisplay(payload.ip),
        reason:
          String(payload.reason) === "cidr_not_allowed"
            ? t("admin.eventCenter.events.sshReasonCidr")
            : t("admin.eventCenter.events.sshReasonThreshold"),
      });
    case "FN_EVENT_SYSTEM_APP_UPDATE_AVAILABLE":
      return t("admin.eventCenter.events.appUpdateAvailable", {
        latest: String(payload.latest_version || "-"),
        current: String(payload.local_version || "-"),
        suffix: payload.force_update
          ? t("admin.eventCenter.events.updateSoonSuffix")
          : "",
      });
    case "FN_EVENT_SYSTEM_CPU_ALERT":
      return t("admin.eventCenter.events.cpuAlert", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_CPU_RECOVERED":
      return t("admin.eventCenter.events.cpuRecovered", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_MEMORY_ALERT":
      return t("admin.eventCenter.events.memoryAlert", {
        hostname: String(payload.hostname || "-"),
        usage: String(payload.usage_percent || "-"),
      });
    case "FN_EVENT_SYSTEM_MEMORY_RECOVERED":
      return t("admin.eventCenter.events.memoryRecovered", {
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
      return t("admin.eventCenter.events.tunnelStatusDescription", {
        tunnel,
        status,
        message: message
          ? t("admin.eventCenter.events.messageSuffix", { message })
          : "",
      });
    }
    default:
      return JSON.stringify(payload);
  }
};

onMounted(() => {
  fetchEvents();
});
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <div class="flex flex-wrap items-center gap-2">
      <SearchInput
        v-model="searchQuery"
        :placeholder="t('admin.eventCenter.events.searchPlaceholder')"
        class="w-full max-w-xs"
        @search="handleSearch"
      />

      <Select v-model="selectedType">
        <SelectTrigger class="w-[160px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.typePlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in TYPE_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Select v-model="selectedLevel">
        <SelectTrigger class="w-[140px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.levelPlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in LEVEL_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Select v-model="selectedSource">
        <SelectTrigger class="w-[110px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.sourcePlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in SOURCE_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div class="ml-auto flex items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="fetchEvents"
        />

        <ConfirmDangerPopover
          v-if="hasSelectedEvents"
          :title="
            t('admin.eventCenter.events.deleteSelectedTitle', {
              count: selectedKeys.size,
            })
          "
          :description="t('admin.eventCenter.events.deleteDescription')"
          :loading="isDeleting"
          :disabled="isDeleting"
          :on-confirm="() => deleteEvents(Array.from(selectedKeys))"
        >
          <template #trigger>
            <Button variant="destructive" :disabled="isDeleting">
              <Trash2 class="mr-2 h-4 w-4" />
              {{
                t("admin.eventCenter.events.deleteSelectedButton", {
                  count: selectedKeys.size,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <div
      class="flex flex-1 flex-col overflow-hidden rounded-md border bg-background"
    >
      <div class="flex-1 overflow-auto">
        <Table
          v-if="!(loading && events.length === 0)"
          class="table-fixed min-w-[980px]"
        >
          <TableHeader class="sticky top-0 z-10 bg-background shadow-sm">
            <TableRow>
              <TableHead class="w-[42px] pl-3 pr-1">
                <Checkbox v-model="isAllSelected" />
              </TableHead>
              <TableHead class="w-[300px]">
                {{ t("admin.eventCenter.events.tableEvent") }}
              </TableHead>
              <TableHead class="w-[220px]">
                {{ t("admin.eventCenter.events.origin") }}
              </TableHead>
              <TableHead class="w-[100px]">
                {{ t("admin.eventCenter.events.level") }}
              </TableHead>
              <TableHead class="w-[96px]">
                {{ t("admin.eventCenter.events.system") }}
              </TableHead>
              <TableHead class="w-[110px] pr-6 text-right">
                {{ t("admin.eventCenter.events.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading">
              <TableCell colspan="6" class="py-10 text-center">
                <Loader2
                  class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                />
              </TableCell>
            </TableRow>
            <TableRow v-else-if="events.length === 0">
              <TableCell
                colspan="6"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.eventCenter.events.empty") }}
              </TableCell>
            </TableRow>
            <TableRow v-for="event in events" :key="event.id">
              <TableCell class="w-[42px] pl-3 pr-1 align-top">
                <Checkbox
                  :model-value="selectedKeys.has(event.id)"
                  @update:model-value="toggleSelect(event.id)"
                />
              </TableCell>
              <TableCell class="w-[340px] max-w-[340px] align-top">
                <div class="space-y-1.5">
                  <div class="flex items-start gap-2">
                    <div
                      class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium leading-5 text-muted-foreground"
                    >
                      <HumanFriendlyTime :value="event.happened_at" />
                    </div>
                    <div
                      class="min-w-0 text-sm font-semibold leading-6"
                      :class="eventTypeTextClass(event)"
                    >
                      {{ formatSystemEventTypeLabel(event.type) }}
                    </div>
                  </div>
                </div>
                <div
                  class="mt-1 max-w-[300px] line-clamp-3 whitespace-normal break-words text-sm leading-6 text-muted-foreground"
                >
                  {{ describeEvent(event) }}
                </div>
              </TableCell>
              <TableCell class="align-middle">
                <div
                  v-if="resolveEventOrigins(event).length === 0"
                  class="text-sm text-muted-foreground"
                >
                  -
                </div>
                <div v-else class="space-y-1">
                  <div
                    v-for="origin in resolveEventOrigins(event)"
                    :key="origin.key"
                    class="space-y-0.5 leading-5"
                  >
                    <div
                      class="font-mono text-xs text-foreground"
                      :title="origin.ip"
                    >
                      {{ formatIpDisplay(origin.ip) }}
                    </div>
                    <div
                      v-if="origin.location"
                      class="line-clamp-2 whitespace-normal text-xs leading-5 text-muted-foreground"
                    >
                      {{ origin.location }}
                    </div>
                  </div>
                </div>
              </TableCell>
              <TableCell>
                <Badge
                  variant="outline"
                  class="border px-2 py-0.5"
                  :class="levelBadgeClass(event.level)"
                >
                  {{ formatSystemEventLevelLabel(event.level) }}
                </Badge>
              </TableCell>
              <TableCell class="truncate align-middle">
                {{ formatSystemEventSourceLabel(event.source) }}
              </TableCell>
              <TableCell class="space-x-2 pr-6 text-right">
                <Button variant="ghost" size="icon" @click="viewDetails(event)">
                  <Eye class="h-4 w-4" />
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.eventCenter.events.deleteSingleTitle')"
                  :description="t('admin.eventCenter.events.deleteDescription')"
                  :loading="isDeleting"
                  :disabled="isDeleting"
                  :on-confirm="() => deleteEvents([event.id])"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="text-destructive"
                      :disabled="isDeleting"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <TableSkeletonBlock
          v-else-if="showTableSkeleton"
          :header-widths="['w-16', 'w-52', 'w-24', 'w-12', 'w-16', 'w-10']"
          :row-widths="['w-16', 'w-56', 'w-28', 'w-12', 'w-20', 'w-10']"
        />

        <div v-else class="h-[420px]" aria-hidden="true"></div>
      </div>

      <PagedTableFooter
        :total="totalEvents"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        :total-text="t('admin.eventCenter.events.totalText')"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.eventCenter.events.detailTitle')"
      :description="t('admin.eventCenter.events.detailDescription')"
      max-width-class="sm:max-w-[760px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEvent" class="space-y-6">
        <DetailFieldsGrid :items="detailItems" />

        <div v-if="activeEvent.tags?.length" class="space-y-2">
          <div class="text-sm font-medium text-foreground">
            {{ t("admin.eventCenter.events.tags") }}
          </div>
          <div class="flex flex-wrap gap-2">
            <Badge
              v-for="tag in activeEvent.tags"
              :key="tag"
              variant="secondary"
              class="rounded-full px-2 py-0.5"
            >
              {{ tag }}
            </Badge>
          </div>
        </div>
      </div>
    </DetailDialog>
  </div>
</template>
