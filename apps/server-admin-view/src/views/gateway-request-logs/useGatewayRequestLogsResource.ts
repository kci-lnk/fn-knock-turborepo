import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { useCursorPagination } from "@/composables/useCursorPagination";
import { useIpLocationBatch } from "@/composables/useIpLocationBatch";
import { ConfigAPI, GatewayLogsAPI } from "@/lib/api";
import { useConfigStore } from "@/store/config";
import type { GatewayLogEntry, TOTPCredential } from "@/types";
import {
  LOGIN_FILTER_OPTIONS,
  STATUS_FILTER_OPTIONS,
  UNRECORDED_CREDENTIAL_FILTER,
  WAF_FILTER_OPTIONS,
  getEntryClientIp,
  getGatewayLogOptionLabel,
  getTodayString,
  type GatewayLoginFilterValue,
  type GatewayStatusFilterValue,
  type GatewayWAFFilterValue,
} from "./model";

export const useGatewayRequestLogsResource = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
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
  const credentialOptions = ref<TOTPCredential[]>([]);
  const selectedLogEntryKeys = ref<Set<string>>(new Set());
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

  const showTableSkeleton = useDelayedLoading(
    () => loading.value && entries.value.length === 0,
  );
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
  const cursorPageLabel = computed(() =>
    t("admin.gatewayRequestLogs.cursorPage", {
      page: cursorHistory.value.length + 1,
    }),
  );
  const shouldFloatPagination = computed(
    () => entries.value.length > 0 || canLoadNewer.value || canLoadOlder.value,
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
    } else if (!nextDates.includes(selectedDate.value)) {
      selectedDate.value = nextDates.includes(fallbackToday)
        ? fallbackToday
        : nextDates[0] || fallbackToday;
    }
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
      trackIps(entries.value.map(getEntryClientIp));
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
    await Promise.all([
      fetchDates(selectedDate.value),
      fetchCredentialOptions(),
    ]);
    resetCursorPagination();
    await fetchEntries();
  };

  const applyFilter = async (update: () => void) => {
    update();
    resetCursorPagination();
    await fetchEntries();
  };

  const handleDateChange = (value: unknown) =>
    value
      ? applyFilter(() => {
          selectedDate.value = String(value);
        })
      : Promise.resolve();
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

  onMounted(async () => {
    await Promise.all([
      fetchDates(selectedDate.value),
      fetchCredentialOptions(),
    ]);
    await fetchEntries();
  });

  return {
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
