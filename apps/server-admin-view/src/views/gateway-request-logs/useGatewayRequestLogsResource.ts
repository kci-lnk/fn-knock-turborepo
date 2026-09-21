import axios from "axios";
import { useGatewayLogDates } from "./useGatewayLogDates";
import { useGatewayLogFilterLabels } from "./useGatewayLogFilterLabels";
import { useRoute } from "vue-router";
import { useGatewayLogIpView } from "./useGatewayLogIpView";
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { useCursorPagination } from "@/composables/useCursorPagination";
import { useIpLocationBatch } from "@/composables/useIpLocationBatch";
import { ConfigAPI } from "@/lib/api/config";
import { GatewayLogsAPI } from "@/lib/api/gateway";
import { useConfigStore } from "@/store/config";
import type { GatewayLogEntry, TOTPCredential } from "@/types";
import {
  getEntryClientIp,
  getTodayString,
  type GatewayLoginFilterValue,
  type GatewayStatusFilterValue,
  type GatewayWAFFilterValue,
} from "./model";
export const useGatewayRequestLogsResource = () => {
  const route = useRoute();
  const configStore = useConfigStore();
  const { t } = useI18n();
  let isDisposed = false;
  let entriesRequestId = 0;
  const entries = ref<GatewayLogEntry[]>([]);
  const logsDir = ref("");
  const availableDates = ref<string[]>([]);
  const selectedDate = ref(
    typeof route.query.log_date === "string"
      ? route.query.log_date
      : getTodayString(),
  );
  const selectedStatus = ref<GatewayStatusFilterValue>("all");
  const selectedLoggedIn = ref<GatewayLoginFilterValue>("all");
  const selectedCredential = ref("all");
  const selectedWAFStatus = ref<GatewayWAFFilterValue>("all");
  const limit = ref("20");
  const searchQuery = ref("");
  const loading = ref(false);
  const credentialOptions = ref<TOTPCredential[]>([]);
  const selectedLogEntryKeys = ref<Set<string>>(new Set());
  const {
    canLoadNewer: canLoadNewerEntries,
    canLoadOlder: canLoadOlderEntries,
    currentCursor,
    cursorHistory,
    loadFirst: loadCursorFirst,
    loadNewer: loadCursorNewer,
    loadOlder: loadCursorOlder,
    nextCursor,
    reset: resetCursorPagination,
  } = useCursorPagination({ loading });
  const { trackIps, getSnapshot } = useIpLocationBatch();
  const ipView = useGatewayLogIpView({
    loading,
    selectedDate,
    searchQuery,
    selectedStatus,
    selectedLoggedIn,
    selectedCredential,
    selectedWAFStatus,
    limit,
    selectedLogEntryKeys,
    resetCursorPagination,
    trackIps,
    invalidateRequest: () => {
      entriesRequestId += 1;
      loading.value = false;
    },
    fetchEntries: () => fetchEntries(),
    applyFilter: (update) => applyFilter(update),
  });
  const {
    clientIp,
    isIpOverview,
    ipGroups,
    ipPage,
    ipSort,
    detailRequests,
    loadError,
  } = ipView;

  const showTableSkeleton = useDelayedLoading(
    () => loading.value && entries.value.length === 0,
  );
  const isLoggingEnabled = computed(
    () => configStore.config?.gateway_logging?.enabled ?? false,
  );
  const {
    activeStatusLabel,
    activeLoggedInLabel,
    credentialFilterOptions,
    activeCredentialLabel,
    activeWAFStatusLabel,
  } = useGatewayLogFilterLabels({
    selectedStatus,
    selectedLoggedIn,
    selectedCredential,
    selectedWAFStatus,
    credentialOptions,
  });
  const canLoadNewer = computed(() =>
    isIpOverview.value ? ipPage.value > 1 : canLoadNewerEntries.value,
  );
  const canLoadOlder = computed(() =>
    isIpOverview.value
      ? ipPage.value * Number(limit.value) < (ipGroups.value?.total ?? 0)
      : canLoadOlderEntries.value,
  );
  const cursorPageLabel = computed(() =>
    t(
      isIpOverview.value
        ? "admin.gatewayRequestLogs.ipView.page"
        : "admin.gatewayRequestLogs.cursorPage",
      {
        page: isIpOverview.value
          ? ipPage.value
          : cursorHistory.value.length + 1,
      },
    ),
  );
  const shouldFloatPagination = computed(
    () =>
      (isIpOverview.value
        ? (ipGroups.value?.items.length ?? 0) > 0
        : entries.value.length > 0) ||
      canLoadNewer.value ||
      canLoadOlder.value,
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

  const { applyDates, fetchDates } = useGatewayLogDates({
    selectedDate,
    availableDates,
    logsDir,
    currentRequest: () => {
      const id = entriesRequestId;
      return () => !isDisposed && id === entriesRequestId;
    },
  });

  const fetchCredentialOptions = async () => {
    try {
      const data = await ConfigAPI.getTOTPStatus();
      if (isDisposed) return;
      credentialOptions.value = data.credentials || [];
    } catch {
      if (isDisposed) return;
      credentialOptions.value = [];
    }
  };

  const fetchEntries = async () => {
    const currentRequestId = ++entriesRequestId;
    const params = {
      date: selectedDate.value,
      pagination: "cursor" as const,
      limit: limit.value,
      cursor: currentCursor.value || undefined,
      search: searchQuery.value || undefined,
      status: selectedStatus.value === "all" ? undefined : selectedStatus.value,
      logged_in:
        selectedLoggedIn.value === "all" ? undefined : selectedLoggedIn.value,
      credential:
        selectedCredential.value === "all"
          ? undefined
          : selectedCredential.value,
      waf_status:
        selectedWAFStatus.value === "all" ? undefined : selectedWAFStatus.value,
      client_ip: clientIp.value || undefined,
    };
    loading.value = true;
    loadError.value = false;
    try {
      if (isIpOverview.value) {
        const data = await GatewayLogsAPI.getIpGroups({
          ...params,
          page: ipPage.value,
          sort: ipSort.value,
        });
        if (isDisposed || currentRequestId !== entriesRequestId) return;
        ipGroups.value = data;
        ipPage.value = data.page;
        trackIps(data.items.map((item) => item.client_ip));
        return;
      }
      const [data, summary] = await Promise.all([
        GatewayLogsAPI.getEntries(params),
        clientIp.value && !currentCursor.value && detailRequests.value === null
          ? GatewayLogsAPI.getIpGroups({ ...params, page: 1, limit: "1" })
          : Promise.resolve(null),
      ]);
      if (isDisposed || currentRequestId !== entriesRequestId) return;
      if (summary) detailRequests.value = summary.total_requests;
      logsDir.value = data.logs_dir || "";
      entries.value = data.items || [];
      selectedLogEntryKeys.value = new Set();
      trackIps(entries.value.map(getEntryClientIp));
      nextCursor.value = data.next_cursor || "";
      applyDates(data.available_dates || [], data.date || params.date);
    } catch (error) {
      if (isDisposed || currentRequestId !== entriesRequestId) return;
      loadError.value = true;
      ipGroups.value = null;
      detailRequests.value = null;
      entries.value = [];
      trackIps([]);
      nextCursor.value = "";
      const cursorExpired =
        axios.isAxiosError(error) && error.response?.status === 409;
      if (cursorExpired) resetCursorPagination();
      toast.error(t("admin.gatewayRequestLogs.loadFailed"), {
        description: cursorExpired
          ? t("admin.gatewayLogging.cursorExpired")
          : extractErrorMessage(
              error,
              t("admin.gatewayRequestLogs.loadFailedDescription"),
            ),
      });
    } finally {
      if (currentRequestId === entriesRequestId) loading.value = false;
    }
  };

  const refreshAll = async () => {
    const requestId = ++entriesRequestId;
    loading.value = true;
    await Promise.all([
      fetchDates(selectedDate.value),
      fetchCredentialOptions(),
    ]);
    if (isDisposed || requestId !== entriesRequestId) return;
    resetCursorPagination();
    ipPage.value = 1;
    detailRequests.value = null;
    await fetchEntries();
  };

  const applyFilter = async (update: () => void) => {
    update();
    resetCursorPagination();
    ipPage.value = 1;
    detailRequests.value = null;
    await fetchEntries();
  };

  const handleDateChange = async (value: unknown) => {
    if (!value) return;
    await applyFilter(() => {
      selectedDate.value = String(value);
    });
    await ipView.syncDateQuery();
  };
  const handleSearch = () => applyFilter(() => undefined);
  const handleStatusChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          selectedStatus.value = String(value) as GatewayStatusFilterValue;
        })
      : Promise.resolve();
  const handleLoggedInChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          selectedLoggedIn.value = String(value) as GatewayLoginFilterValue;
        })
      : Promise.resolve();
  const handleCredentialChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          selectedCredential.value = String(value);
        })
      : Promise.resolve();
  const handleWAFStatusChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          selectedWAFStatus.value = String(value) as GatewayWAFFilterValue;
        })
      : Promise.resolve();
  const handleLimitChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          limit.value = String(value);
        })
      : Promise.resolve();

  const handleLoadOlder = () => ipView.loadPage("older", loadCursorOlder);
  const handleLoadNewer = () => ipView.loadPage("newer", loadCursorNewer);
  const handleLoadFirst = () => ipView.loadPage("first", loadCursorFirst);

  const deleteSelectedDate = async () => {
    await runDelete(() => GatewayLogsAPI.deleteDate(selectedDate.value), {
      onSuccess: async (data) => {
        ipView.invalidateOverview();
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
        await ipView.syncDateQuery();
        await fetchEntries();
      },
    });
  };

  onMounted(async () => {
    const requestId = entriesRequestId;
    loading.value = true;
    await Promise.all([
      fetchDates(
        typeof route.query.log_date === "string"
          ? route.query.log_date
          : undefined,
      ),
      fetchCredentialOptions(),
    ]);
    if (isDisposed || requestId !== entriesRequestId) return;
    await fetchEntries();
  });
  onBeforeUnmount(() => {
    isDisposed = true;
    entriesRequestId += 1;
  });

  return {
    ...ipView,
    retry: fetchEntries,
    activeCredentialLabel,
    activeLoggedInLabel,
    activeStatusLabel,
    activeWAFStatusLabel,
    availableDates,
    canLoadNewer,
    canLoadOlder,
    credentialFilterOptions,
    currentCursor,
    cursorPageLabel,
    deleteSelectedDate,
    entries,
    getSnapshot,
    handleCredentialChange,
    handleDateChange,
    handleLimitChange,
    handleLoadFirst,
    handleLoadNewer,
    handleLoadOlder,
    handleLoggedInChange,
    handleSearch,
    handleStatusChange,
    handleWAFStatusChange,
    isDeleting,
    isLoggingEnabled,
    limit,
    loading,
    logsDir,
    refreshAll,
    searchQuery,
    selectedCredential,
    selectedDate,
    selectedLoggedIn,
    selectedLogEntryKeys,
    selectedStatus,
    selectedWAFStatus,
    shouldFloatPagination,
    showTableSkeleton,
  };
};
