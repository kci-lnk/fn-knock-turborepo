import {
  computed,
  onMounted,
  onUnmounted,
  ref,
  watch,
} from "vue";
import { toast } from "@admin-shared/utils/toast";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import {
  DashboardAPI,
  DDNSAPI,
  SecurityAPI,
  type DDNSStatusPayload,
} from "../../lib/api";
import type { DashboardStats, ThreatOverview } from "../../types";

export const dashboardRanges = [
  {
    key: "15m",
    labelKey: "admin.dashboard.ranges.fifteenMinutes",
    sec: 15 * 60,
  },
  { key: "1h", labelKey: "admin.dashboard.ranges.oneHour", sec: 60 * 60 },
  {
    key: "6h",
    labelKey: "admin.dashboard.ranges.sixHours",
    sec: 6 * 60 * 60,
  },
  {
    key: "1d",
    labelKey: "admin.dashboard.ranges.twentyFourHours",
    sec: 24 * 60 * 60,
  },
  {
    key: "7d",
    labelKey: "admin.dashboard.ranges.sevenDays",
    sec: 7 * 24 * 60 * 60,
  },
] as const;

interface UseDashboardDataOptions {
  disposeTunnelStatus: () => void;
  scheduleTunnelStatusLoad: () => void;
  startRealtimePolling: () => void;
  stopRealtimePolling: () => void;
  translate: (key: string) => string;
}

export function useDashboardData({
  disposeTunnelStatus,
  scheduleTunnelStatusLoad,
  startRealtimePolling,
  stopRealtimePolling,
  translate,
}: UseDashboardDataOptions) {
  const rangeKey = ref<(typeof dashboardRanges)[number]["key"]>("1h");
  const isAutoRefresh = ref(true);
  const { run: runLoadDashboard } = useAsyncAction();
  const isInitializing = ref(true);
  const errorMessage = ref("");
  const stats = ref<DashboardStats | null>(null);
  const threatOverview = ref<ThreatOverview | null>(null);
  const lastUpdatedAt = ref<Date | null>(null);
  const ddnsStatus = ref<DDNSStatusPayload | null>(null);
  const isDdnsInitializing = ref(true);
  const { isPending: isDdnsPending, run: runLoadDdnsStatus } =
    useAsyncAction();
  const ddnsError = ref("");
  let refreshTimer: number | null = null;
  let ddnsLoadTimer: number | null = null;
  let disposed = false;

  const activeRange = computed(
    () =>
      dashboardRanges.find((range) => range.key === rangeKey.value) ??
      dashboardRanges[1],
  );
  const isDdnsLoading = computed(
    () => isDdnsInitializing.value || isDdnsPending.value,
  );
  const showMainSkeleton = useDelayedLoading(isInitializing);
  const showDdnsSkeleton = useDelayedLoading(() => isDdnsLoading.value);

  const loadDdnsStatus = async () => {
    ddnsError.value = "";
    await runLoadDdnsStatus(() => DDNSAPI.getStatus(), {
      onSuccess: (status) => {
        ddnsStatus.value = status;
      },
      onError: (error: any) => {
        ddnsError.value =
          error?.response?.data?.message ||
          error?.message ||
          translate("admin.dashboard.errors.loadFailed");
        ddnsStatus.value = null;
      },
      onFinally: () => {
        isDdnsInitializing.value = false;
      },
    });
  };

  const load = async () => {
    await runLoadDashboard(
      async () => {
        errorMessage.value = "";
        const [statsResult, threatResult] = await Promise.allSettled([
          DashboardAPI.getStats(activeRange.value.sec),
          SecurityAPI.getOverview(activeRange.value.sec),
        ]);
        if (statsResult.status === "fulfilled") {
          stats.value = statsResult.value;
          lastUpdatedAt.value = new Date();
        } else {
          const message =
            (statsResult.reason as any)?.response?.data?.message ||
            (statsResult.reason as any)?.message ||
            translate("admin.dashboard.errors.loadFailed");
          errorMessage.value = message;
          toast.error(translate("admin.dashboard.errors.dashboardLoadFailed"), {
            description: message,
          });
        }
        if (threatResult.status === "fulfilled") {
          threatOverview.value = threatResult.value;
        }
      },
      {
        onError: (error: any) => {
          const message =
            error?.response?.data?.message ||
            error?.message ||
            translate("admin.dashboard.errors.loadFailed");
          errorMessage.value = message;
          toast.error(translate("admin.dashboard.errors.dashboardLoadFailed"), {
            description: message,
          });
        },
        onFinally: () => {
          isInitializing.value = false;
        },
      },
    );
    if (disposed) return;
    scheduleTunnelStatusLoad();
    if (ddnsLoadTimer !== null) window.clearTimeout(ddnsLoadTimer);
    ddnsLoadTimer = window.setTimeout(() => {
      ddnsLoadTimer = null;
      void loadDdnsStatus();
    }, 0);
  };

  const refreshAll = () => void load();
  const startAutoRefresh = () => {
    if (refreshTimer !== null) window.clearInterval(refreshTimer);
    refreshTimer = window.setInterval(() => {
      if (isAutoRefresh.value) refreshAll();
    }, 15000);
  };

  watch(rangeKey, () => void load());
  watch(isAutoRefresh, () => {
    if (isAutoRefresh.value) {
      startAutoRefresh();
    } else if (refreshTimer !== null) {
      window.clearInterval(refreshTimer);
      refreshTimer = null;
    }
  });

  onMounted(() => {
    refreshAll();
    startRealtimePolling();
    if (isAutoRefresh.value) startAutoRefresh();
  });
  onUnmounted(() => {
    disposed = true;
    if (refreshTimer !== null) window.clearInterval(refreshTimer);
    if (ddnsLoadTimer !== null) window.clearTimeout(ddnsLoadTimer);
    refreshTimer = null;
    ddnsLoadTimer = null;
    disposeTunnelStatus();
    stopRealtimePolling();
  });

  return {
    activeRange,
    ddnsError,
    ddnsStatus,
    errorMessage,
    isAutoRefresh,
    isDdnsLoading,
    isInitializing,
    lastUpdatedAt,
    load,
    rangeKey,
    refreshAll,
    showDdnsSkeleton,
    showMainSkeleton,
    stats,
    threatOverview,
  };
}
