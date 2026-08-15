import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { useCursorPagination } from "@/composables/useCursorPagination";
import { useIpLocationBatch } from "@/composables/useIpLocationBatch";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";
import { WAFAPI } from "@/lib/api/gateway";
import { useConfigStore } from "@/store/config";
import type { WAFEvent } from "@/types";

const AUTO_REFRESH_MS = 5_000;
const TRACE_MISS_AUTO_REFRESH_LIMIT = 12;

const getTodayString = () => {
  const now = new Date();
  const year = now.getFullYear();
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");
  return `${year}-${month}-${day}`;
};

export const getWafEventSourceIp = (event: WAFEvent) =>
  event.client_ip || event.remote_addr || "";

export const useWafLogsResource = () => {
  const route = useRoute();
  const configStore = useConfigStore();
  const { t } = useI18n();
  let isDisposed = false;
  let entriesRequestId = 0;
  let traceMissAutoRefreshes = 0;
  const entries = ref<WAFEvent[]>([]);
  const availableDates = ref<string[]>([getTodayString()]);
  const selectedDate = ref(getTodayString());
  const limit = ref("50");
  const searchQuery = ref("");
  const traceFilter = ref(String(route.query.trace_id || ""));
  const loading = ref(false);
  const selectedWafEntryKeys = ref<Set<string>>(new Set());
  const {
    canLoadNewer,
    canLoadOlder,
    currentCursor,
    cursorHistory,
    loadFirst: loadCursorFirst,
    loadNewer: loadCursorNewer,
    loadOlder: loadCursorOlder,
    nextCursor,
    reset: resetCursorPagination,
  } = useCursorPagination({ loading });
  const { trackIps, getSnapshot } = useIpLocationBatch();

  const isWAFEnabled = computed(
    () => configStore.config?.waf?.enabled ?? false,
  );
  const cursorPageLabel = computed(() =>
    t("admin.wafLogs.cursorPage", { page: cursorHistory.value.length + 1 }),
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
    } else if (!nextDates.includes(selectedDate.value)) {
      selectedDate.value = nextDates.includes(fallbackToday)
        ? fallbackToday
        : nextDates[0] || fallbackToday;
    }
  };

  const drainEvents = async (silent = true, signal?: AbortSignal) => {
    try {
      await WAFAPI.drainEvents(signal);
    } catch (error) {
      if (signal?.aborted) return;
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
    options: { silent?: boolean; drain?: boolean; signal?: AbortSignal } = {},
  ) => {
    const currentRequestId = ++entriesRequestId;
    const params = {
      date: selectedDate.value,
      trace_id: traceFilter.value.trim() || undefined,
      search: searchQuery.value.trim() || undefined,
      cursor: currentCursor.value || undefined,
      limit: limit.value,
    };
    const isCurrentRequest = () =>
      !isDisposed &&
      !options.signal?.aborted &&
      currentRequestId === entriesRequestId;
    loading.value = true;
    try {
      if (options.drain) {
        await drainEvents(options.silent !== false, options.signal);
      }
      if (!isCurrentRequest()) return;
      const data = await WAFAPI.getLogs(params, options.signal);
      if (!isCurrentRequest()) return;
      entries.value = data.items || [];
      trackIps(entries.value.map(getWafEventSourceIp));
      nextCursor.value = data.next_cursor || "";
      applyDates(data.available_dates || [], data.date || params.date);
    } catch (error) {
      if (!isCurrentRequest()) return;
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
      if (currentRequestId === entriesRequestId) loading.value = false;
    }
  };

  const refreshAll = async () => {
    traceMissAutoRefreshes = 0;
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
    if (loadCursorOlder()) await fetchEntries();
  };
  const handleLoadNewer = async () => {
    if (loadCursorNewer()) await fetchEntries();
  };
  const handleLoadFirst = async () => {
    if (loadCursorFirst()) await fetchEntries();
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

  const autoRefreshPoller = createVisibilityPoller({
    intervalMs: AUTO_REFRESH_MS,
    immediate: false,
    task: async (signal) => {
      if (currentCursor.value || cursorHistory.value.length > 0) return;
      if (searchQuery.value.trim()) return;
      if (traceFilter.value.trim()) {
        if (
          entries.value.length > 0 ||
          traceMissAutoRefreshes >= TRACE_MISS_AUTO_REFRESH_LIMIT
        ) {
          return;
        }
        traceMissAutoRefreshes += 1;
        await fetchEntries({ silent: true, drain: true, signal });
        return;
      }
      await fetchEntries({ silent: true, signal });
    },
  });

  watch(
    () => route.query.trace_id,
    (value) => {
      const next = String(value || "");
      if (traceFilter.value === next) return;
      traceFilter.value = next;
      traceMissAutoRefreshes = 0;
      resetCursorPagination();
      void fetchEntries({ drain: true });
    },
  );

  onMounted(async () => {
    if (!configStore.config) await configStore.loadConfig();
    await fetchEntries({ drain: true });
    if (isDisposed) return;

    autoRefreshPoller.start();
  });
  onBeforeUnmount(() => {
    isDisposed = true;
    entriesRequestId += 1;
    autoRefreshPoller.stop();
  });

  return {
    availableDates,
    canLoadNewer,
    canLoadOlder,
    currentCursor,
    cursorHistory,
    cursorPageLabel,
    deleteSelectedDate,
    entries,
    getSnapshot,
    handleDateChange,
    handleLimitChange,
    handleLoadFirst,
    handleLoadNewer,
    handleLoadOlder,
    handleSearch,
    isDeleting,
    isWAFEnabled,
    limit,
    loading,
    refreshAll,
    searchQuery,
    selectedDate,
    selectedWafEntryKeys,
    shouldFloatPagination,
    traceFilter,
  };
};
