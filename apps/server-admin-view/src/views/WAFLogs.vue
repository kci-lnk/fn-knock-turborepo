<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  Ban,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Eye,
  Settings,
  ShieldAlert,
  Trash2,
  Unlock,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
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
import { WAFAPI } from "../lib/api";
import type { WAFEvent } from "../types";
import { useConfigStore } from "../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  normalizeIpKey,
  useIpLocationBatch,
} from "../composables/useIpLocationBatch";
import { useWafLogIpSelection } from "./waf-logs/useWafLogIpSelection";
import { useWafLogDisplay } from "./waf-logs/useWafLogDisplay";

const LIMIT_OPTIONS = ["20", "50", "100", "200"] as const;
const AUTO_REFRESH_MS = 5_000;
const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

const route = useRoute();
const router = useRouter();
const configStore = useConfigStore();
const { t, locale } = useI18n();

const entries = ref<WAFEvent[]>([]);
const availableDates = ref<string[]>([getTodayString()]);
const selectedDate = ref(getTodayString());
const limit = ref("50");
const searchQuery = ref("");
const traceFilter = ref(String(route.query.trace_id || ""));
const loading = ref(false);
const isDetailsOpen = ref(false);
const activeEvent = ref<WAFEvent | null>(null);
const selectedWafEntryKeys = ref<Set<string>>(new Set());
const currentCursor = ref("");
const nextCursor = ref("");
const cursorHistory = ref<string[]>([]);
let autoRefreshTimer: number | null = null;

const { trackIps, getSnapshot } = useIpLocationBatch();
const isWAFEnabled = computed(() => configStore.config?.waf?.enabled ?? false);
const canLoadNewer = computed(() => cursorHistory.value.length > 0);
const canLoadOlder = computed(() => Boolean(nextCursor.value));
const cursorPageLabel = computed(
  () => t("admin.wafLogs.cursorPage", { page: cursorHistory.value.length + 1 }),
);
const shouldFloatPagination = computed(
  () => entries.value.length > 0 || canLoadNewer.value || canLoadOlder.value,
);

const { isPending: isDeleting, run: runDelete } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.wafLogs.deleteFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.wafLogs.deleteFailedDescription"),
      ),
    });
  },
});
const applyDates = (dates: string[], preferred?: string) => {
  const fallbackToday = getTodayString();
  const nextDates = dates.length > 0 ? dates : [fallbackToday];
  availableDates.value = nextDates;

  if (preferred && nextDates.includes(preferred)) {
    selectedDate.value = preferred;
    return;
  }
  if (nextDates.includes(selectedDate.value)) return;
  selectedDate.value = nextDates.includes(fallbackToday)
    ? fallbackToday
    : nextDates[0] || fallbackToday;
};

const resetCursorPagination = () => {
  currentCursor.value = "";
  nextCursor.value = "";
  cursorHistory.value = [];
};

const drainEventsSilently = async (silent = true) => {
  try {
    await WAFAPI.drainEvents();
  } catch (error) {
    if (!silent) {
      toast.error(t("admin.wafLogs.drainFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafLogs.drainFailedDescription"),
        ),
      });
    }
  }
};

const fetchEntries = async (
  options: { silent?: boolean; drain?: boolean } = {},
) => {
  if (loading.value) return;
  loading.value = true;
  try {
    if (options.drain) {
      await drainEventsSilently(options.silent !== false);
    }
    const data = await WAFAPI.getLogs({
      date: selectedDate.value,
      trace_id: traceFilter.value.trim() || undefined,
      search: searchQuery.value.trim() || undefined,
      cursor: currentCursor.value || undefined,
      limit: limit.value,
    });
    entries.value = data.items || [];
    trackIps(entries.value.map((entry) => getEntrySourceIp(entry)));
    nextCursor.value = data.next_cursor || "";
    applyDates(data.available_dates || [], data.date || selectedDate.value);
  } catch (error) {
    trackIps([]);
    if (!options.silent) {
      entries.value = [];
      nextCursor.value = "";
      toast.error(t("admin.wafLogs.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafLogs.loadFailedDescription"),
        ),
      });
    }
  } finally {
    loading.value = false;
  }
};

const refreshAll = async () => {
  resetCursorPagination();
  await fetchEntries({ drain: true, silent: false });
};

const handleSearch = async () => {
  resetCursorPagination();
  await fetchEntries();
};

const handleDateChange = async (value: unknown) => {
  if (!value) return;
  selectedDate.value = String(value);
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

const deleteSelectedDate = async () => {
  await runDelete(() => WAFAPI.deleteLogs(selectedDate.value), {
    onSuccess: async (data) => {
      toast.success(
        data.deleted
          ? t("admin.wafLogs.deletedForDate", { date: selectedDate.value })
          : t("admin.wafLogs.noDeletedForDate", {
              date: selectedDate.value,
            }),
      );
      searchQuery.value = "";
      traceFilter.value = "";
      resetCursorPagination();
      applyDates(data.available_dates, getTodayString());
      await fetchEntries();
    },
  });
};

const viewDetails = (event: WAFEvent) => {
  activeEvent.value = event;
  isDetailsOpen.value = true;
};

const goToSettings = () => {
  router.push({ path: "/system", query: { tab: "waf" } });
};

const getEntrySourceIp = (event: WAFEvent) =>
  event.client_ip || event.remote_addr || "";

const getEntryActionIp = (event: WAFEvent) => {
  const sourceIp = getEntrySourceIp(event);
  return normalizeIpKey(sourceIp) || sourceIp.trim();
};

const getEntrySelectionKey = (event: WAFEvent, index: number) =>
  event.trace_id ||
  [
    currentCursor.value || "first",
    index,
    event.time || "",
    event.transaction_id || "",
    event.request_uri || event.path || "",
    getEntryActionIp(event),
  ].join("|");

const getEntryDisplayIp = (event: WAFEvent) => {
  const sourceIp = getEntrySourceIp(event);
  return normalizeIpKey(sourceIp) || sourceIp || "-";
};

const getEntryIpSnapshot = (event: WAFEvent) =>
  getSnapshot(getEntrySourceIp(event));

const getEntryIpLocation = (event: WAFEvent) =>
  getEntryIpSnapshot(event)?.location || "";

const getEntryIpLocationText = (event: WAFEvent) => {
  const snapshot = getEntryIpSnapshot(event);
  const location = snapshot?.location || "";
  if (location) return location;

  if (snapshot?.status === "queued" || snapshot?.status === "processing") {
    return t("admin.hostActiveIps.resolving");
  }

  if (snapshot?.status === "failed") {
    return t("admin.hostActiveIps.unavailable");
  }

  return "";
};

const displayedEntries = computed(() =>
  entries.value.map((entry, index) => ({
    ...entry,
    ipLocation: getEntryIpLocation(entry),
    actionIp: getEntryActionIp(entry),
    selectionKey: getEntrySelectionKey(entry, index),
  })),
);

const {
  blockIpsFromWafLogs,
  hasSelectableDisplayedRows,
  isAllDisplayedRowsSelected,
  isBlockingIps,
  isGeneralBlacklisted,
  isMutatingBlacklistIps,
  isReleasingIps,
  releaseIpsFromWafLogs,
  selectedBlockedWafIps,
  selectedUnblockedWafIps,
  toggleWafEntrySelection,
} = useWafLogIpSelection({
  displayedEntries,
  selectedWafEntryKeys,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const activeEventWithIpLocation = computed(() =>
  activeEvent.value
    ? {
        ...activeEvent.value,
        ipLocation: getEntryIpLocation(activeEvent.value),
      }
    : null,
);

const {
  actionLabel,
  actionVariant,
  detailCopyText,
  detailItems,
  formatPrimaryRuleId,
  formatRuleLocationSummary,
  formatRuleSummary,
  modeLabel,
  routeTypeLabel,
} = useWafLogDisplay({
  activeEvent,
  activeEventWithIpLocation,
  locale,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
watch(
  () => route.query.trace_id,
  (value) => {
    const next = String(value || "");
    if (traceFilter.value === next) return;
    traceFilter.value = next;
    resetCursorPagination();
    void fetchEntries({ drain: true });
  },
);

const startAutoRefresh = () => {
  stopAutoRefresh();
  autoRefreshTimer = window.setInterval(() => {
    if (currentCursor.value || cursorHistory.value.length > 0) return;
    if (searchQuery.value.trim() || traceFilter.value.trim()) return;
    void fetchEntries({ silent: true });
  }, AUTO_REFRESH_MS);
};

const stopAutoRefresh = () => {
  if (autoRefreshTimer !== null) {
    window.clearInterval(autoRefreshTimer);
    autoRefreshTimer = null;
  }
};

onMounted(async () => {
  if (!configStore.config) {
    await configStore.loadConfig();
  }
  await fetchEntries({ drain: true });
  startAutoRefresh();
});

onBeforeUnmount(() => {
  stopAutoRefresh();
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
            {{ t("admin.wafLogs.title") }}
          </h1>
          <span class="text-xs text-muted-foreground">{{ selectedDate }}</span>
        </div>
        <p class="text-sm text-muted-foreground">
          {{ t("admin.wafLogs.description") }}
        </p>
      </div>

      <div class="flex flex-wrap items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          @click="refreshAll"
        />
        <ConfirmDangerPopover
          v-if="selectedUnblockedWafIps.length > 0"
          :title="
            t('admin.wafLogs.blacklistSelectedTitle', {
              count: selectedUnblockedWafIps.length,
            })
          "
          :description="t('admin.wafLogs.blacklistDescription')"
          :loading="isBlockingIps"
          :disabled="
            selectedUnblockedWafIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => blockIpsFromWafLogs(selectedUnblockedWafIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/30 text-destructive hover:bg-destructive/10 hover:text-destructive"
              :disabled="
                selectedUnblockedWafIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Ban class="mr-2 h-4 w-4" />
              {{
                t("admin.wafLogs.blacklistSelected", {
                  count: selectedUnblockedWafIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          v-if="selectedBlockedWafIps.length > 0"
          :title="
            t('admin.wafLogs.unblacklistSelectedTitle', {
              count: selectedBlockedWafIps.length,
            })
          "
          :description="t('admin.wafLogs.unblacklistDescription')"
          :loading="isReleasingIps"
          :disabled="
            selectedBlockedWafIps.length === 0 || isMutatingBlacklistIps
          "
          :on-confirm="() => releaseIpsFromWafLogs(selectedBlockedWafIps)"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="text-foreground"
              :disabled="
                selectedBlockedWafIps.length === 0 || isMutatingBlacklistIps
              "
            >
              <Unlock class="mr-2 h-4 w-4" />
              {{
                t("admin.wafLogs.unblacklistSelected", {
                  count: selectedBlockedWafIps.length,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
        <ConfirmDangerPopover
          :title="t('admin.wafLogs.deleteDateTitle', { date: selectedDate })"
          :description="t('admin.wafLogs.deleteDateDescription')"
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
              {{ t("admin.wafLogs.deleteDateAction") }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <Alert
      v-if="!isWAFEnabled"
      class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
    >
      <ShieldAlert class="h-4 w-4 shrink-0 text-muted-foreground" />
      <div
        class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.wafLogs.disabledNotice") }}
        </p>
        <Button variant="ghost" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          {{ t("admin.wafLogs.goSettings") }}
        </Button>
      </div>
    </Alert>

    <div
      class="flex min-h-0 flex-1 flex-col overflow-hidden rounded-xl border bg-background"
    >
      <div class="border-b px-4 py-3">
        <div class="flex flex-col gap-2 md:flex-row md:items-center">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.wafLogs.searchPlaceholder')"
            class="w-full md:w-[320px] md:max-w-[320px]"
            @search="handleSearch"
          />

          <div class="flex flex-wrap items-center gap-2">
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div class="w-[148px]">
                <SelectTrigger>
                  <SelectValue :placeholder="t('admin.wafLogs.datePlaceholder')" />
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
          </div>
        </div>

        <div
          class="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground"
        >
          <span>
            {{ cursorPageLabel }} ·
            {{ t("admin.wafLogs.rowsCount", { count: entries.length }) }}
          </span>
          <span v-if="traceFilter.trim()" class="font-mono"
            >{{
              t("admin.wafLogs.traceFilter", { trace: traceFilter.trim() })
            }}</span
          >
          <span v-if="searchQuery.trim()"
            >{{
              t("admin.wafLogs.keywordFilter", {
                keyword: searchQuery.trim(),
              })
            }}</span
          >
        </div>
      </div>

      <div class="min-h-0 flex-1 overflow-auto">
        <Table class="min-w-[880px]">
          <TableHeader class="sticky top-0 z-10 bg-background/95 backdrop-blur">
            <TableRow>
              <TableHead
                class="h-10 w-[48px] min-w-[48px] text-[11px] font-medium text-muted-foreground"
              >
                <Checkbox
                  v-model="isAllDisplayedRowsSelected"
                  :disabled="!hasSelectableDisplayedRows"
                />
              </TableHead>
              <TableHead
                class="h-10 w-[320px] min-w-[320px] max-w-[320px] text-[11px] font-medium text-muted-foreground"
                >{{ t("admin.wafLogs.requestColumn") }}</TableHead
              >
              <TableHead
                class="h-10 text-[11px] font-medium text-muted-foreground"
                >{{ t("admin.wafLogs.sourceColumn") }}</TableHead
              >
              <TableHead
                class="h-10 min-w-[220px] text-[11px] font-medium text-muted-foreground"
                >{{ t("admin.wafLogs.rulesColumn") }}</TableHead
              >
              <TableHead
                class="sticky right-0 z-20 h-10 bg-background/95 pr-4 text-right text-[11px] font-medium text-muted-foreground"
                >{{ t("admin.wafLogs.actionColumn") }}</TableHead
              >
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading && entries.length === 0">
              <TableCell
                colspan="5"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.wafLogs.loading") }}
              </TableCell>
            </TableRow>
            <TableRow v-else-if="entries.length === 0">
              <TableCell
                colspan="5"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.wafLogs.empty") }}
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
                  :model-value="selectedWafEntryKeys.has(entry.selectionKey)"
                  :disabled="!entry.actionIp"
                  @update:model-value="
                    toggleWafEntrySelection(entry.selectionKey)
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
                      <HumanFriendlyTime :value="entry.time" :locale="locale" />
                    </div>
                    <Badge
                      :variant="actionVariant(entry.action)"
                      class="shrink-0"
                    >
                      {{ actionLabel(entry.action) }}
                    </Badge>
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
                  <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                    <span class="text-[11px] text-muted-foreground">
                      {{ routeTypeLabel(entry.route_type) }}
                    </span>
                    <span class="text-[11px] text-muted-foreground">
                      {{ modeLabel(entry.mode) }}
                    </span>
                    <span
                      v-if="entry.status"
                      class="font-mono text-[11px] text-muted-foreground"
                    >
                      HTTP {{ entry.status }}
                    </span>
                    <span
                      v-if="entry.route_key"
                      class="break-all text-[11px] text-muted-foreground/75"
                    >
                      {{ entry.route_key }}
                    </span>
                  </div>
                </div>
              </TableCell>
              <TableCell class="min-w-[150px] py-2.5">
                <div class="font-mono text-sm text-foreground">
                  {{ getEntryDisplayIp(entry) }}
                </div>
                <div
                  v-if="getEntryIpLocationText(entry)"
                  class="text-[11px] text-muted-foreground"
                >
                  {{ getEntryIpLocationText(entry) }}
                </div>
              </TableCell>
              <TableCell class="py-2.5">
                <div class="font-mono text-xs text-foreground">
                  {{ formatPrimaryRuleId(entry) }}
                </div>
                <div
                  v-if="formatRuleSummary(entry)"
                  class="mt-1 line-clamp-2 text-[11px] leading-5 text-muted-foreground"
                >
                  {{ formatRuleSummary(entry) }}
                </div>
                <div
                  v-if="formatRuleLocationSummary(entry)"
                  class="mt-1 line-clamp-1 break-all font-mono text-[10px] leading-4 text-muted-foreground/75"
                >
                  {{ formatRuleLocationSummary(entry) }}
                </div>
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
                          ? t('admin.wafLogs.unblacklistOneTitle')
                          : t('admin.wafLogs.blacklistOneTitle')
                      "
                      :description="
                        isGeneralBlacklisted(entry.actionIp)
                          ? t('admin.wafLogs.unblacklistOneDescription', {
                              ip: entry.actionIp || '-',
                            })
                          : t('admin.wafLogs.blacklistOneDescription', {
                              ip: entry.actionIp || '-',
                            })
                      "
                      :loading="isMutatingBlacklistIps"
                      :disabled="!entry.actionIp || isMutatingBlacklistIps"
                      :on-confirm="
                        () =>
                          isGeneralBlacklisted(entry.actionIp)
                            ? releaseIpsFromWafLogs([entry.actionIp])
                            : blockIpsFromWafLogs([entry.actionIp])
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
                              ? t('admin.wafLogs.unblacklistOne')
                              : t('admin.wafLogs.blacklistOne')
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
      </div>

      <FloatingActionDock
        :active="shouldFloatPagination"
        :keep-visible="loading && shouldFloatPagination"
        :keep-visible-release-delay="600"
        align="center"
        variant="surface"
        :visible-threshold="0.4"
        :aria-label="t('admin.wafLogs.title')"
        floating-class="min-w-0 max-w-[calc(100vw-2rem)] rounded-[1.25rem] p-2"
      >
        <template #inline>
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
                    ? t("admin.wafLogs.canLoadOlder")
                    : t("admin.wafLogs.lastPage")
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
                  {{ t("admin.wafLogs.firstPage") }}
                </Button>
                <Button
                  variant="outline"
                  class="h-8 px-3"
                  :disabled="loading || !canLoadNewer"
                  @click="handleLoadNewer"
                >
                  <ChevronLeft class="mr-1.5 h-4 w-4" />
                  {{ t("admin.wafLogs.previousPage") }}
                </Button>
                <Button
                  class="h-8 px-3"
                  :disabled="loading || !canLoadOlder"
                  @click="handleLoadOlder"
                >
                  {{ t("admin.wafLogs.nextPage") }}
                  <ChevronRight class="ml-1.5 h-4 w-4" />
                </Button>

                <div
                  class="ml-1 flex items-center gap-2 text-xs text-muted-foreground"
                >
                  <span>{{ t("admin.wafLogs.pageSize") }}</span>
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
                          t("admin.wafLogs.pageSizeOption", {
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
        </template>

        <template #floating>
          <div class="floating-cursor-pagination">
            <div class="floating-cursor-pagination__controls">
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button"
                :disabled="loading || !canLoadNewer"
                @click="handleLoadFirst"
              >
                <ChevronsLeft class="h-4 w-4" />
                <span class="hidden sm:inline">
                  {{ t("admin.wafLogs.firstPage") }}
                </span>
              </Button>
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button"
                :disabled="loading || !canLoadNewer"
                @click="handleLoadNewer"
              >
                <ChevronLeft class="h-4 w-4" />
                <span class="hidden sm:inline">
                  {{ t("admin.wafLogs.previousPage") }}
                </span>
              </Button>
              <Button
                variant="ghost"
                class="floating-cursor-pagination__button is-primary"
                :disabled="loading || !canLoadOlder"
                @click="handleLoadOlder"
              >
                <span>{{ t("admin.wafLogs.nextPage") }}</span>
                <ChevronRight class="h-4 w-4" />
              </Button>

              <Select
                :model-value="limit"
                @update:model-value="handleLimitChange"
              >
                <SelectTrigger
                  class="h-9 w-[84px] rounded-xl border-white/10 bg-white/10 text-white shadow-none hover:bg-white/15 focus-visible:border-white/30 focus-visible:ring-white/20 [&_svg]:text-white"
                >
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="option in LIMIT_OPTIONS"
                    :key="option"
                    :value="option"
                  >
                    {{ t("admin.wafLogs.pageSizeOption", { count: option }) }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
        </template>
      </FloatingActionDock>
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.wafLogs.detailTitle')"
      :description="t('admin.wafLogs.detailDescription')"
      max-width-class="sm:max-w-[680px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEvent" class="space-y-4">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>

<style scoped>
.floating-cursor-pagination {
  display: flex;
  max-width: calc(100vw - 3rem);
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.6rem 0.8rem;
}

.floating-cursor-pagination__controls {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  justify-content: center;
  gap: 0.35rem;
}

:deep(.floating-cursor-pagination__button) {
  height: 2.25rem;
  min-width: 2.25rem;
  border-color: transparent;
  border-radius: 0.8rem;
  background: transparent;
  padding-inline: 0.7rem;
  color: rgb(255 255 255 / 82%);
  box-shadow: none;
}

:deep(.floating-cursor-pagination__button:hover) {
  background: rgb(255 255 255 / 12%);
  color: #fff;
}

:deep(.floating-cursor-pagination__button.is-primary) {
  background: #fff;
  color: #09090b;
}

:deep(.floating-cursor-pagination__button.is-primary:hover) {
  background: rgb(255 255 255 / 92%);
  color: #09090b;
}

:deep(.floating-cursor-pagination__button:disabled) {
  opacity: 0.46;
}
</style>
