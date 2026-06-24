<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  Info,
  Settings,
  Trash2,
  ChevronLeft,
  ChevronRight,
  ChevronsLeft,
  Ban,
  Unlock,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, GatewayLogsAPI } from "../lib/api";
import type { GatewayLogEntry, TOTPCredential } from "../types";
import { useConfigStore } from "../store/config";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { docsUrls } from "../lib/docs";
import { useIpLocationBatch } from "../composables/useIpLocationBatch";
import {
  LIMIT_OPTIONS,
  LOGIN_FILTER_OPTIONS,
  STATUS_FILTER_OPTIONS,
  UNRECORDED_CREDENTIAL_FILTER,
  WAF_FILTER_OPTIONS,
  buildGatewayLogDetailCopyText,
  buildGatewayLogDetailItems,
  buildGatewayLogSelectionKey,
  getGatewayLogOptionLabel,
  getEntryActionIp,
  getEntryClientIp,
  getTodayString,
  type GatewayLoginFilterValue,
  type GatewayStatusFilterValue,
  type GatewayWAFFilterValue,
} from "./gateway-request-logs/model";
import { useGatewayLogIpSelection } from "./gateway-request-logs/useGatewayLogIpSelection";
import GatewayRequestLogsTable from "./gateway-request-logs/GatewayRequestLogsTable.vue";

const router = useRouter();
const configStore = useConfigStore();
const { t, locale } = useI18n();

const entries = ref<GatewayLogEntry[]>([]);
const logsDir = ref("");
const availableDates = ref<string[]>([]);
const selectedDate = ref(getTodayString());
const selectedStatus = ref<GatewayStatusFilterValue>("all");
const selectedLoggedIn = ref<GatewayLoginFilterValue>("all");
const selectedCredential = ref("all");
const selectedWAFStatus = ref<GatewayWAFFilterValue>("all");
const limit = ref("20");
const searchQuery = ref("");
const loading = ref(false);
const isDetailsOpen = ref(false);
const activeEntry = ref<GatewayLogEntry | null>(null);
const credentialOptions = ref<TOTPCredential[]>([]);
const selectedLogEntryKeys = ref<Set<string>>(new Set());
const currentCursor = ref("");
const nextCursor = ref("");
const cursorHistory = ref<string[]>([]);

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
const normalizedCredentialQuery = computed(() =>
  selectedCredential.value === "all" ? "" : selectedCredential.value,
);
const normalizedWAFStatusQuery = computed(() =>
  selectedWAFStatus.value === "all" ? "" : selectedWAFStatus.value,
);
const activeStatusLabel = computed(() =>
  getGatewayLogOptionLabel(
    STATUS_FILTER_OPTIONS,
    selectedStatus.value,
    "admin.gatewayRequestLogs.statusFilters.all",
    t,
  ),
);
const activeLoggedInLabel = computed(() =>
  getGatewayLogOptionLabel(
    LOGIN_FILTER_OPTIONS,
    selectedLoggedIn.value,
    "admin.gatewayRequestLogs.loginFilters.all",
    t,
  ),
);
const credentialFilterOptions = computed(() => {
  const options = [
    {
      value: "all",
      label: t("admin.gatewayRequestLogs.credentialFilters.all"),
    },
    {
      value: UNRECORDED_CREDENTIAL_FILTER,
      label: t("admin.gatewayRequestLogs.credentialFilters.unrecorded"),
    },
    ...credentialOptions.value.map((credential) => ({
      value: credential.id,
      label: credential.comment?.trim() || credential.id,
    })),
  ];
  if (
    selectedCredential.value !== "all" &&
    !options.some((option) => option.value === selectedCredential.value)
  ) {
    options.push({
      value: selectedCredential.value,
      label: selectedCredential.value,
    });
  }
  return options;
});
const activeCredentialLabel = computed(
  () =>
    credentialFilterOptions.value.find(
      (option) => option.value === selectedCredential.value,
    )?.label || selectedCredential.value,
);
const activeWAFStatusLabel = computed(() =>
  getGatewayLogOptionLabel(
    WAF_FILTER_OPTIONS,
    selectedWAFStatus.value,
    "admin.gatewayRequestLogs.wafFilters.all",
    t,
  ),
);
const canLoadNewer = computed(() => cursorHistory.value.length > 0);
const canLoadOlder = computed(() => Boolean(nextCursor.value));
const cursorPageLabel = computed(() =>
  t("admin.gatewayRequestLogs.cursorPage", {
    page: cursorHistory.value.length + 1,
  }),
);

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

const fetchCredentialOptions = async () => {
  try {
    const data = await ConfigAPI.getTOTPStatus();
    credentialOptions.value = data.credentials || [];
  } catch {
    credentialOptions.value = [];
  }
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
      credential: normalizedCredentialQuery.value || undefined,
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

const refreshAll = async () => {
  await Promise.all([fetchDates(selectedDate.value), fetchCredentialOptions()]);
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
  selectedStatus.value = String(value) as GatewayStatusFilterValue;
  resetCursorPagination();
  await fetchEntries();
};
const handleLoggedInChange = async (value: unknown) => {
  if (!value) return;
  selectedLoggedIn.value = String(value) as GatewayLoginFilterValue;
  resetCursorPagination();
  await fetchEntries();
};

const handleCredentialChange = async (value: unknown) => {
  if (!value) return;
  selectedCredential.value = String(value);
  resetCursorPagination();
  await fetchEntries();
};

const handleWAFStatusChange = async (value: unknown) => {
  if (!value) return;
  selectedWAFStatus.value = String(value) as GatewayWAFFilterValue;
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
      selectedCredential.value = "all";
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

const getEntrySelectionKey = (entry: GatewayLogEntry, index: number) =>
  buildGatewayLogSelectionKey(entry, index, currentCursor.value);

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

const {
  blockIpsFromLogs,
  hasSelectableDisplayedRows,
  isAllDisplayedRowsSelected,
  isBlockingIps,
  isGeneralBlacklisted,
  isMutatingBlacklistIps,
  isReleasingIps,
  releaseIpsFromLogs,
  selectedBlockedLogIps,
  selectedUnblockedLogIps,
  toggleLogEntrySelection,
} = useGatewayLogIpSelection({
  displayedEntries,
  selectedLogEntryKeys,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});

const activeEntryWithIpLocation = computed(() =>
  activeEntry.value
    ? {
        ...activeEntry.value,
        client_ip: getEntryClientIp(activeEntry.value),
        ipLocation: getEntryIpLocation(activeEntry.value),
      }
    : null,
);

const detailItems = computed(() =>
  buildGatewayLogDetailItems(
    activeEntryWithIpLocation.value,
    t,
    String(locale.value),
  ),
);

const detailCopyText = computed(() =>
  buildGatewayLogDetailCopyText(detailItems.value),
);

onMounted(async () => {
  await Promise.all([fetchDates(selectedDate.value), fetchCredentialOptions()]);
  await fetchEntries();
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
        <div class="flex flex-col gap-2 lg:flex-row lg:items-start">
          <SearchInput
            v-model="searchQuery"
            :placeholder="t('admin.gatewayRequestLogs.searchPlaceholder')"
            class="w-full min-w-0 sm:w-[320px] lg:shrink-0"
            @search="handleSearch"
          />

          <div
            class="flex min-w-0 flex-1 flex-wrap items-center gap-2 sm:justify-end"
          >
            <Select
              :model-value="selectedDate"
              @update:model-value="handleDateChange"
            >
              <div class="w-full min-w-0 sm:w-[148px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[156px]">
                <SelectTrigger class="w-full min-w-0">
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
              <div class="w-full min-w-0 sm:w-[168px]">
                <SelectTrigger class="w-full min-w-0">
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
              :model-value="selectedCredential"
              @update:model-value="handleCredentialChange"
            >
              <div class="w-full min-w-0 sm:w-[220px]">
                <SelectTrigger class="w-full min-w-0">
                  <SelectValue
                    :placeholder="
                      t('admin.gatewayRequestLogs.credentialPlaceholder')
                    "
                  />
                </SelectTrigger>
              </div>
              <SelectContent class="max-w-[min(28rem,calc(100vw-2rem))]">
                <SelectItem
                  v-for="option in credentialFilterOptions"
                  :key="option.value"
                  :value="option.value"
                  class="min-w-0"
                >
                  <span
                    class="block max-w-[22rem] truncate"
                    :title="option.label"
                  >
                    {{ option.label }}
                  </span>
                </SelectItem>
              </SelectContent>
            </Select>

            <Select
              :model-value="selectedWAFStatus"
              @update:model-value="handleWAFStatusChange"
            >
              <div class="w-full min-w-0 sm:w-[144px]">
                <SelectTrigger class="w-full min-w-0">
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
          <span class="max-w-[220px] truncate" :title="activeCredentialLabel">
            {{ activeCredentialLabel }}
          </span>
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

      <GatewayRequestLogsTable
        v-model:is-all-displayed-rows-selected="isAllDisplayedRowsSelected"
        :block-ips-from-logs="blockIpsFromLogs"
        :entries="displayedEntries"
        :entries-count="entries.length"
        :get-connection-source-text="getConnectionSourceText"
        :get-entry-ip-location-text="getEntryIpLocationText"
        :go-to-waf-trace="goToWAFTrace"
        :has-selectable-displayed-rows="hasSelectableDisplayedRows"
        :is-general-blacklisted="isGeneralBlacklisted"
        :is-mutating-blacklist-ips="isMutatingBlacklistIps"
        :loading="loading"
        :release-ips-from-logs="releaseIpsFromLogs"
        :selected-log-entry-keys="selectedLogEntryKeys"
        :show-table-skeleton="showTableSkeleton"
        :toggle-log-entry-selection="toggleLogEntrySelection"
        :view-details="viewDetails"
      />

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
