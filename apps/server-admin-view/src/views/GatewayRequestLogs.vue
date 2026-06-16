<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Info,
  Eye,
  Settings,
  Trash2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Ban,
  Unlock,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
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
import { toast } from "@admin-shared/utils/toast";
import { GatewayLogsAPI, GeneralBlacklistAPI } from "../lib/api";
import type { GatewayLogEntry } from "../types";
import { useConfigStore } from "../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { buildDetailFields } from "@admin-shared/utils/buildDetailFields";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { docsUrls } from "../lib/docs";
import {
  normalizeIpKey,
  useIpLocationBatch,
} from "../composables/useIpLocationBatch";
import { useGeneralBlacklistStatus } from "../composables/useGeneralBlacklistStatus";

const router = useRouter();
const configStore = useConfigStore();
const { t, locale } = useI18n();

const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

const LIMIT_OPTIONS = ["10", "20", "50", "100"] as const;
const STATUS_FILTER_OPTIONS = [
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
const LOGIN_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.loginFilters.all" },
  { value: "true", labelKey: "admin.gatewayRequestLogs.loginFilters.loggedIn" },
  {
    value: "false",
    labelKey: "admin.gatewayRequestLogs.loginFilters.notLoggedIn",
  },
] as const;
const WAF_FILTER_OPTIONS = [
  { value: "all", labelKey: "admin.gatewayRequestLogs.wafFilters.all" },
  { value: "has_waf", labelKey: "admin.gatewayRequestLogs.wafFilters.hasWaf" },
  { value: "none", labelKey: "admin.gatewayRequestLogs.wafFilters.none" },
] as const;

const entries = ref<GatewayLogEntry[]>([]);
const logsDir = ref("");
const availableDates = ref<string[]>([]);
const selectedDate = ref(getTodayString());
const selectedStatus =
  ref<(typeof STATUS_FILTER_OPTIONS)[number]["value"]>("all");
const selectedLoggedIn =
  ref<(typeof LOGIN_FILTER_OPTIONS)[number]["value"]>("all");
const selectedWAFStatus =
  ref<(typeof WAF_FILTER_OPTIONS)[number]["value"]>("all");
const limit = ref("20");
const searchQuery = ref("");
const loading = ref(false);
const isDetailsOpen = ref(false);
const activeEntry = ref<GatewayLogEntry | null>(null);
const selectedLogEntryKeys = ref<Set<string>>(new Set());
const currentCursor = ref("");
const nextCursor = ref("");
const cursorHistory = ref<string[]>([]);
const tableScrollRef = ref<HTMLElement | null>(null);
const topScrollbarRef = ref<HTMLElement | null>(null);
const tableContentWidth = ref(0);
const tableViewportWidth = ref(0);
const tableScrollLeft = ref(0);

const showTableSkeleton = useDelayedLoading(
  () => loading.value && entries.value.length === 0,
);
const { trackIps, getSnapshot } = useIpLocationBatch();
const isLoggingEnabled = computed(
  () => configStore.config?.gateway_logging?.enabled ?? false,
);
const normalizedStatusQuery = computed(() =>
  selectedStatus.value === "all" ? "" : selectedStatus.value,
);
const normalizedLoggedInQuery = computed(() =>
  selectedLoggedIn.value === "all" ? "" : selectedLoggedIn.value,
);
const normalizedWAFStatusQuery = computed(() =>
  selectedWAFStatus.value === "all" ? "" : selectedWAFStatus.value,
);
const activeStatusLabel = computed(() =>
  t(
    STATUS_FILTER_OPTIONS.find((item) => item.value === selectedStatus.value)
      ?.labelKey || "admin.gatewayRequestLogs.statusFilters.all",
  ),
);
const activeLoggedInLabel = computed(() =>
  t(
    LOGIN_FILTER_OPTIONS.find((item) => item.value === selectedLoggedIn.value)
      ?.labelKey || "admin.gatewayRequestLogs.loginFilters.all",
  ),
);
const activeWAFStatusLabel = computed(() =>
  t(
    WAF_FILTER_OPTIONS.find((item) => item.value === selectedWAFStatus.value)
      ?.labelKey || "admin.gatewayRequestLogs.wafFilters.all",
  ),
);
const hasHorizontalOverflow = computed(
  () => tableContentWidth.value > tableViewportWidth.value + 1,
);
const canScrollLeft = computed(
  () => hasHorizontalOverflow.value && tableScrollLeft.value > 1,
);
const canScrollRight = computed(
  () =>
    hasHorizontalOverflow.value &&
    tableScrollLeft.value + tableViewportWidth.value <
      tableContentWidth.value - 1,
);
const canLoadNewer = computed(() => cursorHistory.value.length > 0);
const canLoadOlder = computed(() => Boolean(nextCursor.value));
const cursorPageLabel = computed(() =>
  t("admin.gatewayRequestLogs.cursorPage", {
    page: cursorHistory.value.length + 1,
  }),
);

let resizeObserver: ResizeObserver | null = null;
let isSyncingHorizontalScroll = false;

const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayRequestLogs.deleteFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayRequestLogs.deleteFailedDescription"),
      ),
    });
  },
});
const { isPending: isBlockingIps, run: runBlockIps } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayRequestLogs.blacklistFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayRequestLogs.blacklistFailed"),
      ),
    });
  },
});
const { isPending: isReleasingIps, run: runReleaseIps } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.gatewayRequestLogs.unblacklistFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayRequestLogs.unblacklistFailed"),
      ),
    });
  },
});
const isMutatingBlacklistIps = computed(
  () => isBlockingIps.value || isReleasingIps.value,
);

const applyDates = (dates: string[], preferred?: string) => {
  const fallbackToday = getTodayString();
  const nextDates = dates.length > 0 ? dates : [fallbackToday];
  availableDates.value = nextDates;

  if (preferred && nextDates.includes(preferred)) {
    selectedDate.value = preferred;
    return;
  }
  if (nextDates.includes(selectedDate.value)) {
    return;
  }
  if (nextDates.includes(fallbackToday)) {
    selectedDate.value = fallbackToday;
    return;
  }
  selectedDate.value = nextDates[0] || fallbackToday;
};

const fetchDates = async (preferred?: string) => {
  const data = await GatewayLogsAPI.getDates();
  logsDir.value = data.logs_dir || "";
  applyDates(data.dates || [], preferred || data.today || selectedDate.value);
};

const fetchEntries = async () => {
  loading.value = true;
  try {
    const data = await GatewayLogsAPI.getEntries({
      date: selectedDate.value,
      pagination: "cursor",
      limit: limit.value,
      cursor: currentCursor.value || undefined,
      search: searchQuery.value || undefined,
      status: normalizedStatusQuery.value || undefined,
      logged_in: normalizedLoggedInQuery.value || undefined,
      waf_status: normalizedWAFStatusQuery.value || undefined,
    });
    logsDir.value = data.logs_dir || "";
    entries.value = data.items || [];
    selectedLogEntryKeys.value = new Set();
    trackIps(entries.value.map((entry) => getEntryClientIp(entry)));
    nextCursor.value = data.next_cursor || "";
    applyDates(data.available_dates || [], data.date || selectedDate.value);
  } catch (error) {
    entries.value = [];
    trackIps([]);
    nextCursor.value = "";
    toast.error(t("admin.gatewayRequestLogs.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.gatewayRequestLogs.loadFailedDescription"),
      ),
    });
  } finally {
    loading.value = false;
  }
};

const updateHorizontalOverflow = () => {
  const scrollEl = tableScrollRef.value;
  tableViewportWidth.value = scrollEl?.clientWidth || 0;
  tableContentWidth.value = scrollEl?.scrollWidth || 0;
  tableScrollLeft.value = scrollEl?.scrollLeft || 0;
};

const syncHorizontalScroll = (source: "table" | "top") => {
  if (isSyncingHorizontalScroll) return;

  const tableEl = tableScrollRef.value;
  const topEl = topScrollbarRef.value;
  if (!tableEl || !topEl) return;

  isSyncingHorizontalScroll = true;
  if (source === "table") {
    topEl.scrollLeft = tableEl.scrollLeft;
  } else {
    tableEl.scrollLeft = topEl.scrollLeft;
  }

  requestAnimationFrame(() => {
    isSyncingHorizontalScroll = false;
  });
};

const bindTableResizeObserver = () => {
  resizeObserver?.disconnect();
  resizeObserver = null;

  if (typeof ResizeObserver === "undefined" || !tableScrollRef.value) {
    updateHorizontalOverflow();
    return;
  }

  resizeObserver = new ResizeObserver(() => {
    updateHorizontalOverflow();
  });

  resizeObserver.observe(tableScrollRef.value);

  const tableEl = tableScrollRef.value.querySelector("table");
  if (tableEl instanceof HTMLElement) {
    resizeObserver.observe(tableEl);
  }

  updateHorizontalOverflow();
};

const refreshAll = async () => {
  await fetchDates(selectedDate.value);
  currentCursor.value = "";
  nextCursor.value = "";
  cursorHistory.value = [];
  await fetchEntries();
};

const resetCursorPagination = () => {
  currentCursor.value = "";
  nextCursor.value = "";
  cursorHistory.value = [];
};

const handleDateChange = async (value: unknown) => {
  if (!value) return;
  selectedDate.value = String(value);
  resetCursorPagination();
  await fetchEntries();
};

const handleSearch = async () => {
  resetCursorPagination();
  await fetchEntries();
};

const handleStatusChange = async (value: unknown) => {
  if (!value) return;
  selectedStatus.value = String(
    value,
  ) as (typeof STATUS_FILTER_OPTIONS)[number]["value"];
  resetCursorPagination();
  await fetchEntries();
};
const handleLoggedInChange = async (value: unknown) => {
  if (!value) return;
  selectedLoggedIn.value = String(
    value,
  ) as (typeof LOGIN_FILTER_OPTIONS)[number]["value"];
  resetCursorPagination();
  await fetchEntries();
};

const handleWAFStatusChange = async (value: unknown) => {
  if (!value) return;
  selectedWAFStatus.value = String(
    value,
  ) as (typeof WAF_FILTER_OPTIONS)[number]["value"];
  resetCursorPagination();
  await fetchEntries();
};

const handleLimitChange = async (value: unknown) => {
  if (!value) return;
  limit.value = String(value);
  resetCursorPagination();
  await fetchEntries();
};

const handleLoadOlder = async () => {
  if (!nextCursor.value || loading.value) return;
  cursorHistory.value = [...cursorHistory.value, currentCursor.value];
  currentCursor.value = nextCursor.value;
  await fetchEntries();
};

const handleLoadNewer = async () => {
  if (cursorHistory.value.length === 0 || loading.value) return;
  const history = [...cursorHistory.value];
  const previousCursor = history.pop() ?? "";
  cursorHistory.value = history;
  currentCursor.value = previousCursor;
  await fetchEntries();
};

const handleLoadFirst = async () => {
  if (cursorHistory.value.length === 0 || loading.value) return;
  resetCursorPagination();
  await fetchEntries();
};

const viewDetails = (entry: GatewayLogEntry) => {
  activeEntry.value = entry;
  isDetailsOpen.value = true;
};

const deleteSelectedDate = async () => {
  await runDelete(() => GatewayLogsAPI.deleteDate(selectedDate.value), {
    onSuccess: async (data) => {
      toast.success(
        data.deleted
          ? t("admin.gatewayRequestLogs.deletedForDate", {
              date: selectedDate.value,
            })
          : t("admin.gatewayRequestLogs.noDeletedForDate", {
              date: selectedDate.value,
            }),
      );
      searchQuery.value = "";
      selectedStatus.value = "all";
      selectedLoggedIn.value = "all";
      selectedWAFStatus.value = "all";
      resetCursorPagination();
      const nextPreferred =
        data.available_dates.find((item) => item !== selectedDate.value) ||
        getTodayString();
      await fetchDates(nextPreferred);
      await fetchEntries();
    },
  });
};

const goToSettings = () => {
  router.push({ path: "/system", query: { tab: "gateway-logging" } });
};

const goToWAFTrace = (traceId?: string) => {
  if (!traceId) return;
  router.push({ path: "/waf-logs", query: { trace_id: traceId } });
};

const wafActionLabel = (value?: string) => {
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

const wafModeLabel = (value?: string) => {
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

const formatRuleIds = (value?: number[]) =>
  value && value.length > 0 ? value.join(", ") : "-";

const hasWAFSignal = (entry: GatewayLogEntry) =>
  Boolean(entry.waf_trace_id) ||
  Boolean(entry.waf_bundle) ||
  Boolean(entry.waf_action) ||
  entry.waf_blocked === true ||
  (Array.isArray(entry.waf_rule_ids) && entry.waf_rule_ids.length > 0);

const getWAFAction = (entry: GatewayLogEntry) =>
  String(entry.waf_action || "").toLowerCase();

const isWAFBlocked = (entry: GatewayLogEntry) =>
  entry.waf_blocked === true ||
  getWAFAction(entry) === "block" ||
  getWAFAction(entry) === "deny";

const wafBadgeLabel = (entry: GatewayLogEntry) => {
  if (isWAFBlocked(entry))
    return t("admin.gatewayRequestLogs.wafBadges.blocked");
  const action = getWAFAction(entry);
  if (action === "pass") return t("admin.gatewayRequestLogs.wafBadges.pass");
  if (action === "log" || action === "detect")
    return t("admin.gatewayRequestLogs.wafBadges.record");
  return t("admin.gatewayRequestLogs.wafBadges.hit");
};

const wafBadgeClass = (entry: GatewayLogEntry) => {
  if (isWAFBlocked(entry)) {
    return "border-red-500/20 bg-transparent text-red-600/80 hover:bg-red-500/[0.04] dark:text-red-300/80";
  }
  if (getWAFAction(entry) === "pass") {
    return "border-emerald-500/20 bg-transparent text-emerald-600/80 hover:bg-emerald-500/[0.04] dark:text-emerald-300/80";
  }
  return "border-muted-foreground/20 bg-transparent text-muted-foreground hover:bg-muted/30";
};

const wafBadgeMeta = (entry: GatewayLogEntry) => {
  if (entry.waf_rule_ids?.length) {
    return entry.waf_rule_ids.map((id) => `#${id}`).join(" ");
  }
  return entry.waf_trace_id || wafActionLabel(entry.waf_action);
};

const wafBadgeTitle = (entry: GatewayLogEntry) => {
  const parts = [wafBadgeLabel(entry)];
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

const statusTextClass = (status: number) => {
  if (status >= 500) return "text-red-600";
  if (status >= 400) return "text-amber-600";
  return "text-foreground";
};

const statusDotClass = (status: number) => {
  if (status >= 500) return "bg-red-500";
  if (status >= 400) return "bg-amber-500";
  return "bg-muted-foreground/35";
};

const routeTypeLabel = (value?: string) => {
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

const authDecisionLabel = (value?: string) => {
  switch (value) {
    case "passed":
      return t("admin.gatewayRequestLogs.authDecisions.passed");
    case "redirected":
      return t("admin.gatewayRequestLogs.authDecisions.redirected");
    case "denied":
      return t("admin.gatewayRequestLogs.authDecisions.denied");
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

const formatDuration = (value?: number) => {
  if (!Number.isFinite(value)) return "-";
  return `${value} ms`;
};

const formatBoolean = (value?: boolean) => {
  return value
    ? t("admin.gatewayRequestLogs.boolean.yes")
    : t("admin.gatewayRequestLogs.boolean.no");
};

const formatDate = (value?: string) =>
  formatDateTimeSafe(value, { locale: locale.value });

const getEntryClientIp = (entry: GatewayLogEntry) =>
  entry.client_ip || entry.remote_ip || "";

const getEntryActionIp = (entry: GatewayLogEntry) => {
  const clientIp = getEntryClientIp(entry);
  return normalizeIpKey(clientIp) || clientIp.trim();
};

const getEntrySelectionKey = (entry: GatewayLogEntry, index: number) =>
  [
    currentCursor.value || "first",
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

const getEntryIpSnapshot = (entry: GatewayLogEntry) =>
  getSnapshot(getEntryClientIp(entry));

const getEntryIpLocation = (entry: GatewayLogEntry) =>
  getEntryIpSnapshot(entry)?.location || entry.ipLocation || "";

const getEntryIpLocationText = (entry: GatewayLogEntry) => {
  const snapshot = getEntryIpSnapshot(entry);
  const location = snapshot?.location || entry.ipLocation || "";
  if (location) return location;

  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return t("admin.hostActiveIps.resolving");
  }

  if (snapshot?.status === "failed") {
    return t("admin.hostActiveIps.unavailable");
  }

  return "";
};

const getForwardedHeaderLines = (entry: GatewayLogEntry) => {
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

const getConnectionSourceText = (entry: GatewayLogEntry) => {
  const clientIp = getEntryClientIp(entry);
  const remoteIp = entry.remote_ip || "";
  if (!remoteIp || remoteIp === clientIp) return "";
  return t("admin.gatewayRequestLogs.connectionSource", { ip: remoteIp });
};

const displayedEntries = computed(() =>
  entries.value.map((entry, index) => ({
    ...entry,
    client_ip: getEntryClientIp(entry),
    ipLocation: getEntryIpLocation(entry),
    actionIp: getEntryActionIp(entry),
    selectionKey: getEntrySelectionKey(entry, index),
  })),
);

const displayedEntryKeys = computed(() =>
  displayedEntries.value.map((entry) => entry.selectionKey),
);

const displayedSelectableEntryKeys = computed(() =>
  displayedEntries.value
    .filter((entry) => entry.actionIp)
    .map((entry) => entry.selectionKey),
);

const displayedEntryIps = computed(() =>
  Array.from(
    new Set(
      displayedEntries.value
        .map((entry) => entry.actionIp)
        .filter(Boolean),
    ),
  ),
);
const {
  refresh: refreshGeneralBlacklistStatus,
  isBlacklisted: isGeneralBlacklisted,
} = useGeneralBlacklistStatus(displayedEntryIps);
const selectedLogIpList = computed(() =>
  Array.from(
    new Set(
      displayedEntries.value
        .filter((entry) => selectedLogEntryKeys.value.has(entry.selectionKey))
        .map((entry) => entry.actionIp)
        .filter(Boolean),
    ),
  ),
);
const selectedBlockedLogIps = computed(() =>
  selectedLogIpList.value.filter((ip) => isGeneralBlacklisted(ip)),
);
const selectedUnblockedLogIps = computed(() =>
  selectedLogIpList.value.filter((ip) => !isGeneralBlacklisted(ip)),
);

const isAllDisplayedRowsSelected = computed({
  get: () =>
    displayedSelectableEntryKeys.value.length > 0 &&
    displayedSelectableEntryKeys.value.every((key) =>
      selectedLogEntryKeys.value.has(key),
    ),
  set: (checked: boolean) => {
    const next = new Set(selectedLogEntryKeys.value);
    if (checked) {
      displayedEntries.value.forEach((entry) => {
        if (entry.actionIp) next.add(entry.selectionKey);
      });
    } else {
      displayedEntryKeys.value.forEach((key) => next.delete(key));
    }
    selectedLogEntryKeys.value = next;
  },
});

const toggleLogEntrySelection = (key?: string) => {
  if (!key) return;
  const next = new Set(selectedLogEntryKeys.value);
  if (next.has(key)) {
    next.delete(key);
  } else {
    next.add(key);
  }
  selectedLogEntryKeys.value = next;
};

const removeSelectedLogIps = (ips: string[]) => {
  const operatedIps = new Set(ips);
  selectedLogEntryKeys.value = new Set(
    displayedEntries.value
      .filter(
        (entry) =>
          selectedLogEntryKeys.value.has(entry.selectionKey) &&
          !operatedIps.has(entry.actionIp),
      )
      .map((entry) => entry.selectionKey),
  );
};

const blockIpsFromLogs = async (ips: string[]) => {
  const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter(
    (ip) => !isGeneralBlacklisted(ip),
  );
  if (uniqueIps.length === 0) return;

  await runBlockIps(() => GeneralBlacklistAPI.add(uniqueIps, "request_log"), {
    onSuccess: async (result) => {
      toast.success(t("admin.gatewayRequestLogs.blacklistSuccess"), {
        description: t("admin.gatewayRequestLogs.blacklistSuccessDetail", {
          added: result?.added ?? 0,
          updated: result?.updated ?? 0,
        }),
      });
      removeSelectedLogIps(uniqueIps);
      await refreshGeneralBlacklistStatus();
    },
  });
};

const releaseIpsFromLogs = async (ips: string[]) => {
  const uniqueIps = Array.from(new Set(ips.filter(Boolean))).filter((ip) =>
    isGeneralBlacklisted(ip),
  );
  if (uniqueIps.length === 0) return;

  await runReleaseIps(() => GeneralBlacklistAPI.delete(uniqueIps), {
    onSuccess: async (result) => {
      toast.success(t("admin.gatewayRequestLogs.unblacklistSuccess"), {
        description: t("admin.gatewayRequestLogs.unblacklistSuccessDetail", {
          removed: result?.removed ?? 0,
        }),
      });
      removeSelectedLogIps(uniqueIps);
      await refreshGeneralBlacklistStatus();
    },
  });
};

const activeEntryWithIpLocation = computed(() =>
  activeEntry.value
    ? {
        ...activeEntry.value,
        client_ip: getEntryClientIp(activeEntry.value),
        ipLocation: getEntryIpLocation(activeEntry.value),
      }
    : null,
);

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

const localizedDetailFields = computed(() =>
  detailFields.map((field) => ({
    key: field.key,
    label: "label" in field ? field.label : t(field.labelKey),
  })),
);

const detailItems = computed(() =>
  buildDetailFields(
    activeEntryWithIpLocation.value,
    localizedDetailFields.value,
    {
      format: (key, value) => {
        if (key === "time") return formatDate(value);
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
          return formatBoolean(Boolean(value));
        }
        if (key === "route_type") return routeTypeLabel(String(value || ""));
        if (key === "auth_decision")
          return authDecisionLabel(String(value || ""));
        if (key === "waf_action") return wafActionLabel(String(value || ""));
        if (key === "waf_mode") return wafModeLabel(String(value || ""));
        if (key === "waf_rule_ids") return formatRuleIds(value as number[]);
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

watch(
  [entries, loading],
  async () => {
    await nextTick();
    bindTableResizeObserver();
  },
  { flush: "post" },
);

onMounted(async () => {
  await fetchDates(selectedDate.value);
  await fetchEntries();
  await nextTick();
  bindTableResizeObserver();
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = null;
});
</script>

<template>
  <div class="flex h-full flex-col gap-3">
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <div class="flex items-center gap-2">
          <h1 class="text-lg font-semibold tracking-tight">
            {{ t("admin.gatewayRequestLogs.title") }}
          </h1>
          <span class="text-xs text-muted-foreground">{{ selectedDate }}</span>
        </div>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.description") }}
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <DocsLinkButton :href="docsUrls.guides.requestLogs" />
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="refreshAll"
        />
        <ConfirmDangerPopover
          v-if="selectedUnblockedLogIps.length > 0"
          :title="
            t('admin.gatewayRequestLogs.blacklistSelectedTitle', {
              count: selectedUnblockedLogIps.length,
            })
          "
          :description="t('admin.gatewayRequestLogs.blacklistDescription')"
          :loading="isBlockingIps"
          :disabled="
            selectedUnblockedLogIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => blockIpsFromLogs(selectedUnblockedLogIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="
                selectedUnblockedLogIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Ban class="mr-2 h-4 w-4" />
              {{
                t("admin.gatewayRequestLogs.blacklistSelected", {
                  count: selectedUnblockedLogIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          v-if="selectedBlockedLogIps.length > 0"
          :title="
            t('admin.gatewayRequestLogs.unblacklistSelectedTitle', {
              count: selectedBlockedLogIps.length,
            })
          "
          :description="t('admin.gatewayRequestLogs.unblacklistDescription')"
          :loading="isReleasingIps"
          :disabled="
            selectedBlockedLogIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => releaseIpsFromLogs(selectedBlockedLogIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="text-foreground"
              :disabled="
                selectedBlockedLogIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Unlock class="mr-2 h-4 w-4" />
              {{
                t("admin.gatewayRequestLogs.unblacklistSelected", {
                  count: selectedBlockedLogIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          :title="
            t('admin.gatewayRequestLogs.deleteDateTitle', {
              date: selectedDate,
            })
          "
          :description="t('admin.gatewayRequestLogs.deleteDateDescription')"
          :loading="isDeleting"
          :disabled="isDeleting"
          :on-confirm="deleteSelectedDate"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="isDeleting"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.gatewayRequestLogs.deleteDateAction") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <Alert
      v-if="!isLoggingEnabled"
      class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
    >
      <Info class="h-4 w-4 shrink-0 text-muted-foreground" />
      <div
        class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.disabledNotice") }}
        </p>
        <Button variant="ghost" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          {{ t("admin.gatewayRequestLogs.goSettings") }}
        </Button>
      </div>
    </Alert>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <div class="border-b px-4 py-3">
        <div class="flex flex-col gap-2 xl:flex-row xl:items-center">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.gatewayRequestLogs.searchPlaceholder')"
            class="w-full xl:w-[320px] xl:max-w-[320px]"
            @search="handleSearch"
          />

          <div class="flex flex-1 flex-wrap items-center gap-2">
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div class="w-[148px]">
                <SelectTrigger>
                  <SelectValue
                    :placeholder="t('admin.gatewayRequestLogs.datePlaceholder')"
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="date in availableDates"
                  :key="date"
                  :value="date"
                >
                  {{ date }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedStatus"
              @update:model-value="handleStatusChange"
            >
              <div class="w-[156px]">
                <SelectTrigger>
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.statusPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in STATUS_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedLoggedIn"
              @update:model-value="handleLoggedInChange"
            >
              <div class="w-[156px]">
                <SelectTrigger>
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.loginPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in LOGIN_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedWAFStatus"
              @update:model-value="handleWAFStatusChange"
            >
              <div class="w-[148px]">
                <SelectTrigger>
                  <SelectValue
                    :placeholder="t('admin.gatewayRequestLogs.wafPlaceholder')"
                  />
                </SelectTrigger>
              </div>
              <SelectContent>
                <SelectItem
                  v-for="option in WAF_FILTER_OPTIONS"
                  :key="option.value"
                  :value="option.value"
                >
                  {{ t(option.labelKey) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <div
          class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
        >
          <span>
            {{ cursorPageLabel }} ·
            {{
              t("admin.gatewayRequestLogs.rowsCount", {
                count: entries.length,
              })
            }}
          </span>
          <span>{{ activeStatusLabel }}</span>
          <span>{{ activeLoggedInLabel }}</span>
          <span>{{ activeWAFStatusLabel }}</span>
          <span v-if="searchQuery.trim()">{{
            t("admin.gatewayRequestLogs.keywordFilter", {
              keyword: searchQuery.trim(),
            })
          }}</span>
          <span class="break-all">{{
            t("admin.gatewayRequestLogs.directoryLabel", {
              directory: logsDir || "-",
            })
          }}</span>
        </div>
      </div>

      <div v-if="hasHorizontalOverflow" class="border-b px-4 py-2">
        <div
          ref="topScrollbarRef"
          class="overflow-x-auto overscroll-x-contain rounded-full bg-muted/35 p-1"
          @scroll="syncHorizontalScroll('top')"
        >
          <div
            class="h-1.5 rounded-full bg-foreground/20"
            :style="{
              width: `${Math.max(tableContentWidth, tableViewportWidth)}px`,
            }"
          ></div>
        </div>
      </div>

      <div class="relative flex-1 overflow-hidden">
        <div
          v-if="canScrollLeft"
          class="pointer-events-none absolute inset-y-0 left-0 z-10 w-6 bg-gradient-to-r from-background to-transparent"
        ></div>
        <div
          v-if="canScrollRight"
          class="pointer-events-none absolute inset-y-0 right-0 z-10 w-6 bg-gradient-to-l from-background to-transparent"
        ></div>

        <div
          ref="tableScrollRef"
          class="h-full overflow-auto overscroll-x-contain"
          @scroll="syncHorizontalScroll('table')"
        >
          <Table
            v-if="!(loading && entries.length === 0)"
            class="min-w-[1040px]"
          >
            <TableHeader
              class="sticky top-0 z-10 bg-background/95 backdrop-blur"
            >
              <TableRow>
                <TableHead
                  class="h-10 w-[48px] min-w-[48px] text-[11px] font-medium text-muted-foreground"
                >
                  <Checkbox
                    v-model="isAllDisplayedRowsSelected"
                    :disabled="displayedSelectableEntryKeys.length === 0"
                  />
                </TableHead>
                <TableHead
                  class="h-10 w-[320px] min-w-[320px] max-w-[320px] text-[11px] font-medium text-muted-foreground"
                  >{{
                    t("admin.gatewayRequestLogs.columns.request")
                  }}</TableHead
                >
                <TableHead
                  class="h-10 text-[11px] font-medium text-muted-foreground"
                  >{{ t("admin.gatewayRequestLogs.columns.status") }}</TableHead
                >
                <TableHead
                  class="h-10 text-[11px] font-medium text-muted-foreground"
                  >{{ t("admin.gatewayRequestLogs.columns.login") }}</TableHead
                >
                <TableHead
                  class="h-10 text-[11px] font-medium text-muted-foreground"
                  >{{
                    t("admin.gatewayRequestLogs.columns.clientIp")
                  }}</TableHead
                >
                <TableHead
                  class="h-10 text-[11px] font-medium text-muted-foreground"
                  >{{ t("admin.gatewayRequestLogs.columns.route") }}</TableHead
                >
                <TableHead
                  class="h-10 text-[11px] font-medium text-muted-foreground"
                  >{{
                    t("admin.gatewayRequestLogs.columns.duration")
                  }}</TableHead
                >
                <TableHead
                  class="sticky right-0 z-20 h-10 bg-background/95 pr-4 text-right text-[11px] font-medium text-muted-foreground"
                  >{{
                    t("admin.gatewayRequestLogs.columns.actions")
                  }}</TableHead
                >
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell
                  colspan="8"
                  class="py-10 text-center text-muted-foreground"
                >
                  {{ t("admin.gatewayRequestLogs.loading") }}
                </TableCell>
              </TableRow>
              <TableRow v-else-if="entries.length === 0">
                <TableCell
                  colspan="8"
                  class="py-10 text-center text-muted-foreground"
                >
                  {{ t("admin.gatewayRequestLogs.empty") }}
                </TableCell>
              </TableRow>
              <TableRow
                v-else
                v-for="entry in displayedEntries"
                :key="entry.selectionKey"
                class="group align-top"
              >
                <TableCell class="py-2.5">
                  <Checkbox
                    :model-value="selectedLogEntryKeys.has(entry.selectionKey)"
                    :disabled="!entry.actionIp"
                    @update:model-value="
                      toggleLogEntrySelection(entry.selectionKey)
                    "
                  />
                </TableCell>
                <TableCell
                  class="w-[320px] min-w-[320px] max-w-[320px] whitespace-normal py-2.5"
                >
                  <div class="space-y-1.5">
                    <div class="flex items-start gap-2">
                      <div
                        class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium leading-5 text-muted-foreground"
                      >
                        <HumanFriendlyTime
                          :value="entry.time"
                          :locale="locale"
                        />
                      </div>
                      <div class="min-w-0 flex-1">
                        <div
                          class="flex items-center gap-2 text-sm text-foreground"
                        >
                          <span
                            class="font-mono text-[11px] tracking-[0.12em] text-muted-foreground"
                          >
                            {{ entry.method || "-" }}
                          </span>
                          <span class="min-w-0 flex-1 truncate">{{
                            entry.host || "-"
                          }}</span>
                        </div>
                      </div>
                    </div>
                    <div
                      class="whitespace-normal break-all font-mono text-[11px] leading-5 text-muted-foreground"
                    >
                      {{ entry.request_uri || entry.path || "-" }}
                    </div>
                    <div
                      v-if="entry.upstream"
                      class="whitespace-normal break-all text-[11px] text-muted-foreground/75"
                    >
                      {{ entry.upstream }}
                    </div>
                    <button
                      v-if="hasWAFSignal(entry)"
                      type="button"
                      class="inline-flex max-w-full items-center gap-1 rounded-full border px-1.5 py-px text-[10px] font-normal leading-4 transition-colors disabled:cursor-default disabled:opacity-70"
                      :class="wafBadgeClass(entry)"
                      :title="wafBadgeTitle(entry)"
                      :disabled="!entry.waf_trace_id"
                      @click.stop="goToWAFTrace(entry.waf_trace_id)"
                    >
                      <ShieldX
                        v-if="isWAFBlocked(entry)"
                        class="h-2.5 w-2.5 shrink-0"
                      />
                      <ShieldCheck
                        v-else-if="getWAFAction(entry) === 'pass'"
                        class="h-2.5 w-2.5 shrink-0"
                      />
                      <ShieldAlert v-else class="h-2.5 w-2.5 shrink-0" />
                      <span class="shrink-0">{{ wafBadgeLabel(entry) }}</span>
                      <span class="truncate font-mono">{{
                        wafBadgeMeta(entry)
                      }}</span>
                    </button>
                  </div>
                </TableCell>
                <TableCell class="py-2.5">
                  <div
                    class="flex items-center gap-2 font-mono text-sm"
                    :class="statusTextClass(entry.status)"
                  >
                    <span
                      class="h-1.5 w-1.5 rounded-full"
                      :class="statusDotClass(entry.status)"
                    ></span>
                    <span>{{ entry.status }}</span>
                  </div>
                </TableCell>
                <TableCell class="py-2.5">
                  <div class="text-sm text-foreground">
                    {{
                      entry.logged_in
                        ? t("admin.gatewayRequestLogs.loggedIn")
                        : t("admin.gatewayRequestLogs.notLoggedIn")
                    }}
                  </div>
                  <div class="text-[11px] text-muted-foreground">
                    {{ authDecisionLabel(entry.auth_decision) }}
                  </div>
                </TableCell>
                <TableCell class="min-w-[140px] py-2.5">
                  <div class="font-mono text-sm text-foreground">
                    {{ getEntryClientIp(entry) || "-" }}
                  </div>
                  <div
                    v-if="getConnectionSourceText(entry)"
                    class="break-all text-[10px] text-muted-foreground/75"
                  >
                    {{ getConnectionSourceText(entry) }}
                  </div>
                  <div
                    v-if="getEntryIpLocationText(entry)"
                    class="text-[11px] text-muted-foreground"
                  >
                    {{ getEntryIpLocationText(entry) }}
                  </div>
                  <div
                    v-for="headerLine in getForwardedHeaderLines(entry)"
                    :key="headerLine"
                    class="break-all text-[10px] text-muted-foreground/75"
                  >
                    {{ headerLine }}
                  </div>
                </TableCell>
                <TableCell class="min-w-[110px] py-2.5">
                  <div class="text-sm text-foreground">
                    {{ routeTypeLabel(entry.route_type) }}
                  </div>
                  <div class="break-all text-[11px] text-muted-foreground">
                    {{ entry.route_key || "-" }}
                  </div>
                </TableCell>
                <TableCell
                  class="whitespace-nowrap py-2.5 font-mono text-sm text-muted-foreground"
                >
                  {{ formatDuration(entry.duration_ms) }}
                </TableCell>
                <TableCell
                  class="sticky right-0 z-10 bg-background py-2.5 pr-4 text-right"
                >
                  <div class="flex justify-end gap-1">
                    <div
                      class="pointer-events-none opacity-0 transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100"
                    >
                      <ConfirmDangerPopover
                        :title="
                          isGeneralBlacklisted(entry.actionIp)
                            ? t('admin.gatewayRequestLogs.unblacklistOneTitle')
                            : t('admin.gatewayRequestLogs.blacklistOneTitle')
                        "
                        :description="
                          isGeneralBlacklisted(entry.actionIp)
                            ? t(
                                'admin.gatewayRequestLogs.unblacklistOneDescription',
                                {
                                  ip: entry.actionIp || '-',
                                },
                              )
                            : t('admin.gatewayRequestLogs.blacklistOneDescription', {
                                ip: entry.actionIp || '-',
                              })
                        "
                        :loading="isMutatingBlacklistIps"
                        :disabled="!entry.actionIp || isMutatingBlacklistIps"
                        :on-confirm="
                          () =>
                            isGeneralBlacklisted(entry.actionIp)
                              ? releaseIpsFromLogs([entry.actionIp])
                              : blockIpsFromLogs([entry.actionIp])
                        "
                      >
                        <template #trigger>
                          <Button
                            variant="ghost"
                            size="icon"
                            class="h-8 w-8"
                            :class="
                              isGeneralBlacklisted(entry.actionIp)
                                ? 'text-foreground hover:text-foreground'
                                : 'text-destructive hover:text-destructive'
                            "
                            :disabled="!entry.actionIp || isMutatingBlacklistIps"
                            :aria-label="
                              isGeneralBlacklisted(entry.actionIp)
                                ? t('admin.gatewayRequestLogs.unblacklistOne')
                                : t('admin.gatewayRequestLogs.blacklistOne')
                            "
                          >
                            <Unlock
                              v-if="isGeneralBlacklisted(entry.actionIp)"
                              class="h-4 w-4"
                            />
                            <Ban v-else class="h-4 w-4" />
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
                    </div>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="h-8 w-8 text-muted-foreground hover:text-foreground"
                      :aria-label="t('common.viewDetails')"
                      @click="viewDetails(entry)"
                    >
                      <Eye class="h-4 w-4" />
                    </Button>
                  </div>
                </TableCell>
              </TableRow>
            </TableBody>
          </Table>
          <TableSkeletonBlock
            v-else-if="showTableSkeleton"
            :header-widths="[
              'w-4',
              'w-56',
              'w-16',
              'w-16',
              'w-20',
              'w-20',
              'w-14',
              'w-10',
            ]"
            :row-widths="[
              'w-4',
              'w-64',
              'w-12',
              'w-20',
              'w-24',
              'w-24',
              'w-14',
              'w-10',
            ]"
          />
          <div v-else class="h-[380px]" aria-hidden="true"></div>
        </div>
      </div>

      <div class="border-t px-4 py-3">
        <div
          class="flex flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
        >
          <div
            class="flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-muted-foreground"
          >
            <span>{{ cursorPageLabel }}</span>
            <span>{{
              canLoadOlder
                ? t("admin.gatewayRequestLogs.canLoadOlder")
                : t("admin.gatewayRequestLogs.lastPage")
            }}</span>
          </div>

          <div class="flex flex-wrap items-center justify-end gap-2">
            <Button
              variant="outline"
              class="h-8 px-3"
              :disabled="loading || !canLoadNewer"
              @click="handleLoadFirst"
            >
              <ChevronsLeft class="mr-1.5 h-4 w-4" />
              {{ t("admin.gatewayRequestLogs.firstPage") }}
            </Button>
            <Button
              variant="outline"
              class="h-8 px-3"
              :disabled="loading || !canLoadNewer"
              @click="handleLoadNewer"
            >
              <ChevronLeft class="mr-1.5 h-4 w-4" />
              {{ t("admin.gatewayRequestLogs.previousPage") }}
            </Button>
            <Button
              class="h-8 px-3"
              :disabled="loading || !canLoadOlder"
              @click="handleLoadOlder"
            >
              {{ t("admin.gatewayRequestLogs.nextPage") }}
              <ChevronRight class="ml-1.5 h-4 w-4" />
            </Button>

            <div
              class="ml-1 flex items-center gap-2 text-xs text-muted-foreground"
            >
              <span>{{ t("admin.gatewayRequestLogs.pageSize") }}</span>
              <Select
                :model-value="limit"
                @update:model-value="handleLimitChange"
              >
                <div class="w-[96px]">
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                </div>
                <SelectContent>
                  <SelectItem
                    v-for="option in LIMIT_OPTIONS"
                    :key="option"
                    :value="option"
                  >
                    {{
                      t("admin.gatewayRequestLogs.pageSizeOption", {
                        count: option,
                      })
                    }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </div>
      </div>
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.gatewayRequestLogs.detailTitle')"
      :description="t('admin.gatewayRequestLogs.detailDescription')"
      max-width-class="sm:max-w-[640px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEntry">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>
