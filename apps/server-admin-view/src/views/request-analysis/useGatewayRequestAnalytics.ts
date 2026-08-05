import {
  computed,
  onActivated,
  onBeforeUnmount,
  onDeactivated,
  onMounted,
  ref,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { GatewayLogsAPI } from "@/lib/api";
import type { GatewayLogAnalyticsPayload } from "@/types";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  isValidAnalyticsRange,
  resolveAnalyticsRange,
  todayDateString,
  type RequestAnalyticsRangeKey,
} from "./model";

const GEO_POLL_INTERVAL_MS = 5000;
const GEO_MAX_POLLS = 12;
const RANGE_KEYS = new Set<RequestAnalyticsRangeKey>(["today", "7d", "30d"]);

const readQueryValue = (value: unknown) =>
  Array.isArray(value) ? String(value[0] || "") : String(value || "");

export const useGatewayRequestAnalytics = () => {
  const { t } = useI18n();
  const route = useRoute();
  const router = useRouter();
  const queryRange = readQueryValue(
    route.query.range,
  ) as RequestAnalyticsRangeKey;
  const rangeKey = ref<RequestAnalyticsRangeKey>(
    RANGE_KEYS.has(queryRange) ? queryRange : "7d",
  );

  const data = ref<GatewayLogAnalyticsPayload | null>(null);
  const loading = ref(false);
  const loadFailed = ref(false);
  const geoRefreshStarting = ref(false);
  const gatewayToday = ref(todayDateString());
  let activeRequestId = 0;
  let geoPolls = 0;
  let isActive = true;
  let refreshOnActivate = false;
  let pollTimer: ReturnType<typeof window.setTimeout> | null = null;

  const activeRange = computed(() =>
    resolveAnalyticsRange(rangeKey.value, gatewayToday.value),
  );
  const geoRefreshing = computed(
    () =>
      geoRefreshStarting.value ||
      Boolean(data.value?.geo.refreshing) ||
      data.value?.geo.status === "resolving",
  );

  const clearPollTimer = () => {
    if (pollTimer) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const syncRangeQuery = () => {
    const nextQuery: Record<string, string | string[] | null | undefined> = {
      ...route.query,
      range: rangeKey.value,
    };
    delete nextQuery.from;
    delete nextQuery.to;
    void router.replace({ query: nextQuery });
  };

  const scheduleGeoPoll = () => {
    clearPollTimer();
    const geo = data.value?.geo;
    if (
      !isActive ||
      !geo ||
      (!geo.refreshing &&
        (geo.status !== "resolving" || geoPolls >= GEO_MAX_POLLS))
    ) {
      return;
    }
    pollTimer = window.setTimeout(() => {
      geoPolls += 1;
      void loadAnalytics(true);
    }, GEO_POLL_INTERVAL_MS);
  };

  const loadAnalytics = async (silent = false, resetData = false) => {
    const range = activeRange.value;
    if (!isValidAnalyticsRange(range.from, range.to, gatewayToday.value)) {
      return;
    }
    const requestId = ++activeRequestId;
    if (!silent) {
      loadFailed.value = false;
      loading.value = true;
      if (resetData) data.value = null;
    }
    try {
      const result = await GatewayLogsAPI.getAnalytics(range);
      if (requestId !== activeRequestId) return;
      data.value = result;
      loadFailed.value = false;
      scheduleGeoPoll();
    } catch (error) {
      if (requestId !== activeRequestId) return;
      if (silent) {
        scheduleGeoPoll();
        return;
      }
      loadFailed.value = true;
      toast.error(t("admin.requestAnalysis.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.requestAnalysis.loadFailedDescription"),
        ),
      });
    } finally {
      if (requestId === activeRequestId && !silent) loading.value = false;
    }
  };

  const selectRange = (value: string | number) => {
    const next = String(value) as RequestAnalyticsRangeKey;
    if (!RANGE_KEYS.has(next)) return;
    if (next === rangeKey.value) return;
    rangeKey.value = next;
    geoPolls = 0;
    syncRangeQuery();
    void loadAnalytics(false, true);
  };

  const refresh = () => {
    geoPolls = 0;
    clearPollTimer();
    void loadAnalytics();
  };

  const refreshGeo = async () => {
    if (geoRefreshing.value) return;
    geoRefreshStarting.value = true;
    try {
      await GatewayLogsAPI.refreshAnalyticsGeo(activeRange.value);
      if (data.value) data.value.geo.refreshing = true;
      geoPolls = 0;
      scheduleGeoPoll();
    } catch (error) {
      await loadAnalytics(true);
      toast.error(t("admin.requestAnalysis.geo.refreshFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.requestAnalysis.geo.refreshFailedDescription"),
        ),
      });
    } finally {
      geoRefreshStarting.value = false;
    }
  };

  const initialize = async () => {
    loading.value = true;
    try {
      const dates = await GatewayLogsAPI.getDates();
      if (isValidAnalyticsRange(dates.today, dates.today, dates.today)) {
        gatewayToday.value = dates.today;
      }
    } catch {
      // The analytics endpoint remains usable when the lightweight dates
      // request fails, so fall back to the browser's local calendar date.
    }
    syncRangeQuery();
    await loadAnalytics();
  };

  onMounted(() => {
    void initialize();
  });
  watch(
    () => [route.query.range, route.query.from, route.query.to],
    () => {
      const queryKey = readQueryValue(
        route.query.range,
      ) as RequestAnalyticsRangeKey;
      const nextKey = RANGE_KEYS.has(queryKey) ? queryKey : "7d";
      const shouldNormalizeQuery =
        !RANGE_KEYS.has(queryKey) ||
        Boolean(route.query.from || route.query.to);
      if (nextKey === rangeKey.value) {
        if (shouldNormalizeQuery) syncRangeQuery();
        return;
      }
      rangeKey.value = nextKey;
      geoPolls = 0;
      clearPollTimer();
      if (shouldNormalizeQuery) syncRangeQuery();
      void loadAnalytics(false, true);
    },
  );
  onActivated(() => {
    isActive = true;
    if (refreshOnActivate && data.value) {
      refreshOnActivate = false;
      geoPolls = 0;
      void loadAnalytics(true);
      return;
    }
    scheduleGeoPoll();
  });
  onDeactivated(() => {
    isActive = false;
    refreshOnActivate = true;
    clearPollTimer();
  });
  onBeforeUnmount(() => {
    isActive = false;
    clearPollTimer();
  });

  return {
    activeRange,
    data,
    geoRefreshing,
    loadFailed,
    loading,
    rangeKey,
    refresh,
    refreshGeo,
    selectRange,
  };
};
