import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  ScannerAPI,
  SecurityAPI,
  type ScannerBlacklistRecord,
} from "@/lib/api/security";
import {
  DEFAULT_THREAT_RANGES,
  useThreatOverview,
} from "@admin-shared/composables/useThreatOverview";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import { toast } from "@admin-shared/utils/toast";
import type { TimeSeriesChartSeries } from "@/components/charts/TimeSeriesChart.vue";
import { useScannerFalsePositive } from "./useScannerFalsePositive";

export function useIpBlacklistPage() {
  const { t, locale } = useI18n();
  const router = useRouter();
  const ranges = DEFAULT_THREAT_RANGES;

  const formatOverviewRangeText = (seconds: number) => {
    if (seconds < 3_600) {
      return t("admin.components.threatOverview.rangeMinutes", {
        count: Math.round(seconds / 60),
      });
    }
    if (seconds < 24 * 3_600) {
      return t("admin.components.threatOverview.rangeHours", {
        count: Math.round(seconds / 3_600),
      });
    }
    return t("admin.components.threatOverview.rangeDays", {
      count: Math.round(seconds / 86_400),
    });
  };

  const {
    rangeKey,
    threatOverview,
    isThreatLoading,
    titleRangeText,
    perHour: blockedPerHour,
    formatNumber,
    formatRate,
    fetchThreatOverview,
  } = useThreatOverview({
    defaultRangeKey: "1h",
    ranges,
    seriesKey: "blockedScanners",
    fetchOverview: (rangeSec) => SecurityAPI.getOverview(rangeSec),
    onError: (error: unknown) => {
      toast.error(t("admin.sessions.ipBlacklist.threatLoadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.ipBlacklist.loadFailed"),
        ),
      });
    },
    formatRangeText: formatOverviewRangeText,
    numberLocale: () => locale.value,
  });

  const blockedTrendSeries = computed<TimeSeriesChartSeries[]>(() => [
    {
      name: t("admin.sessions.ipBlacklist.seriesName"),
      color: "#f97316",
      fill: "rgba(249, 115, 22, 0.14)",
      data: threatOverview.value?.series.blockedScanners ?? [],
    },
  ]);

  const isDetailsModalOpen = ref(false);
  const detailRecord = ref<ScannerBlacklistRecord | null>(null);
  const { isPending: isDeleting, run: runDeleteAction } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessions.ipBlacklist.deleteFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.ipBlacklist.deleteFailed"),
        ),
      });
    },
  });
  const { isPending: isDetailLoading, run: runLoadDetail } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessions.ipBlacklist.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.ipBlacklist.detailLoadFailed"),
        ),
      });
      detailRecord.value = null;
    },
  });

  const {
    items: records,
    total: totalRecords,
    loading,
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    selectedKeys: selectedIps,
    isAllSelected,
    fetchList: fetchBlacklist,
    handleSearch,
    handlePageChange,
    handleLimitChange,
    toggleSelect,
    clearSelection,
  } = usePagedSelectionList<ScannerBlacklistRecord, string>({
    fetchPage: async ({ page, limit: pageLimit, query }) => {
      const data = await ScannerAPI.getBlacklist(page, pageLimit, query);
      return {
        items: data.items || [],
        total: data.total || 0,
      };
    },
    getKey: (record) => record.ip,
    onError: (error: unknown) => {
      toast.error(t("admin.sessions.ipBlacklist.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.ipBlacklist.loadFailed"),
        ),
      });
    },
  });

  const { isResolvingFalsePositive, resolveFalsePositive } =
    useScannerFalsePositive({
      clearSelection,
      detailRecord,
      isDetailsModalOpen,
      fetchBlacklist,
    });

  const showTableSkeleton = useDelayedLoading(
    () => loading.value && records.value.length === 0,
  );

  const deleteBlacklist = async (ips: string[]) => {
    if (ips.length === 0) return;
    await runDeleteAction(() => ScannerAPI.deleteBlacklist(ips), {
      onSuccess: async () => {
        toast.success(t("admin.sessions.ipBlacklist.deleteSuccess"));
        clearSelection();
        await fetchBlacklist();
      },
    });
  };

  const deleteOne = async (ip: string) => {
    await runDeleteAction(() => ScannerAPI.deleteBlacklistByIp(ip), {
      onSuccess: async () => {
        toast.success(t("admin.sessions.ipBlacklist.deleteSuccess"));
        selectedIps.value.delete(ip);
        selectedIps.value = new Set(selectedIps.value);
        await fetchBlacklist();
      },
    });
  };

  const viewDetails = async (record: ScannerBlacklistRecord) => {
    isDetailsModalOpen.value = true;
    await runLoadDetail(() => ScannerAPI.getBlacklistDetail(record.ip), {
      onSuccess: (detail) => {
        detailRecord.value = detail;
      },
    });
  };

  const formatDate = (timestamp?: number) =>
    formatDateTimeSafe(timestamp, { locale: locale.value });

  const formatIntervalSeconds = (value: number | null) => {
    if (value === null || !Number.isFinite(value)) return "-";
    return t("admin.sessions.ipBlacklist.seconds", {
      seconds: (value * 60).toFixed(2),
    });
  };

  const detailHitRows = computed(() => {
    const sortedHits = [...(detailRecord.value?.hits ?? [])].sort(
      (left, right) => left.createdAt - right.createdAt,
    );
    return sortedHits.map((hit, index) => {
      const previous = sortedHits[index - 1];
      const intervalMinutes = previous
        ? (hit.createdAt - previous.createdAt) / 60_000
        : null;
      return {
        key: `${hit.createdAt}-${index}`,
        time: formatDate(hit.createdAt),
        path: hit.path,
        interval: formatIntervalSeconds(intervalMinutes),
      };
    });
  });

  const goToFirewallSettings = () => {
    void router.push({ path: "/system", query: { tab: "scanner-firewall" } });
  };

  onMounted(() => {
    void fetchBlacklist();
    void fetchThreatOverview();
  });

  return {
    blockedPerHour,
    blockedTrendSeries,
    currentPage,
    deleteBlacklist,
    deleteOne,
    detailHitRows,
    detailRecord,
    fetchBlacklist,
    formatDate,
    formatNumber,
    formatRate,
    goToFirewallSettings,
    handleLimitChange,
    handlePageChange,
    handleSearch,
    isAllSelected,
    isDeleting,
    isDetailLoading,
    isDetailsModalOpen,
    isResolvingFalsePositive,
    isThreatLoading,
    limit,
    loading,
    parsedLimit,
    rangeKey,
    ranges,
    records,
    resolveFalsePositive,
    searchQuery,
    selectedIps,
    showTableSkeleton,
    threatOverview,
    titleRangeText,
    toggleSelect,
    totalRecords,
    viewDetails,
  };
}

export type IpBlacklistPageController = ReturnType<typeof useIpBlacklistPage>;
