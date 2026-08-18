import { computed, ref, watch, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import type { TimeSeriesChartSeries } from "@/components/charts/TimeSeriesChart.vue";
import { DashboardAPI } from "@/lib/api/dashboard";
import type { DashboardStats, StreamTrafficStats } from "@/types";

const rangeDefs = [
  { key: "15m", sec: 15 * 60 },
  { key: "1h", sec: 60 * 60 },
  { key: "6h", sec: 6 * 60 * 60 },
  { key: "1d", sec: 24 * 60 * 60 },
  { key: "7d", sec: 7 * 24 * 60 * 60 },
] as const;

type RangeKey = (typeof rangeDefs)[number]["key"];

const normalizeSeriesData = (value: unknown) => {
  if (!Array.isArray(value)) return [];
  return value
    .map((point) => {
      if (!Array.isArray(point)) return null;
      const time = Number(point[0]);
      const amount = Number(point[1]);
      if (!Number.isFinite(time) || !Number.isFinite(amount)) return null;
      return [time, amount] as const;
    })
    .filter((point): point is readonly [number, number] => Boolean(point));
};

export const useStreamTrafficStats = (options: {
  active: Readonly<Ref<boolean>>;
  stream: Readonly<Ref<string>>;
  sample: Readonly<Ref<StreamTrafficStats | null>>;
  timestamp: Readonly<Ref<number | null>>;
}) => {
  const { t } = useI18n();
  const rangeKey = ref<RangeKey>("1h");
  const stats = ref<DashboardStats | null>(null);
  const isStatsLoading = ref(false);
  const statsError = ref("");
  const realtimeInBps = ref<number | null>(null);
  const realtimeOutBps = ref<number | null>(null);
  let statsRequestId = 0;
  let lastRealtimeSample: {
    at: number;
    totalIn: number;
    totalOut: number;
  } | null = null;

  const formatPlainRangeText = (seconds: number) => {
    if (seconds < 3600) {
      return t("admin.hostTraffic.plainMinutes", {
        count: Math.round(seconds / 60),
      });
    }
    if (seconds < 24 * 3600) {
      return t("admin.hostTraffic.plainHours", {
        count: Math.round(seconds / 3600),
      });
    }
    return t("admin.hostTraffic.plainDays", {
      count: Math.round(seconds / 86400),
    });
  };

  const ranges = computed(() =>
    rangeDefs.map((range) => ({
      ...range,
      label: formatPlainRangeText(range.sec),
    })),
  );
  const activeRange = computed(
    () =>
      rangeDefs.find((range) => range.key === rangeKey.value) ?? rangeDefs[1]!,
  );

  const formatBytes = (bytes: number | null | undefined) => {
    const value = Number(bytes ?? 0);
    if (!Number.isFinite(value) || value <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"] as const;
    const exp = Math.max(
      0,
      Math.min(units.length - 1, Math.floor(Math.log(value) / Math.log(1024))),
    );
    const displayValue = value / 1024 ** exp;
    const digits =
      exp === 0 ? 0 : displayValue >= 100 ? 0 : displayValue >= 10 ? 1 : 2;
    return `${displayValue.toFixed(digits)} ${units[exp] ?? "B"}`;
  };

  const formatBps = (bps: number | null | undefined) =>
    bps === null || bps === undefined ? "-" : `${formatBytes(bps)} /s`;

  const hasRealtimeInTraffic = computed(
    () => Number(realtimeInBps.value ?? 0) > 0,
  );
  const hasRealtimeOutTraffic = computed(
    () => Number(realtimeOutBps.value ?? 0) > 0,
  );
  const hasCompactTraffic = computed(
    () => hasRealtimeInTraffic.value || hasRealtimeOutTraffic.value,
  );
  const compactInText = computed(() => formatBps(realtimeInBps.value));
  const compactOutText = computed(() => formatBps(realtimeOutBps.value));
  const realtimeInText = computed(() => formatBps(realtimeInBps.value));
  const realtimeOutText = computed(() => formatBps(realtimeOutBps.value));
  const rangeText = computed(() =>
    formatPlainRangeText(stats.value?.rangeSec ?? activeRange.value.sec),
  );

  const trafficSeries = computed<TimeSeriesChartSeries[]>(() => {
    const base = (stats.value?.traffic.echarts ?? {}) as any;
    const colors = ["#047857", "#1d4ed8"];
    return (Array.isArray(base?.series) ? base.series : []).map(
      (item: any, index: number) => {
        const color = colors[index % colors.length] ?? "#047857";
        return {
          name: String(item?.name ?? ""),
          color,
          fill: `${color}14`,
          data: normalizeSeriesData(item?.data),
        };
      },
    );
  });

  const loadStats = async () => {
    const requestId = ++statsRequestId;
    isStatsLoading.value = true;
    statsError.value = "";
    try {
      const result = await DashboardAPI.getStats(activeRange.value.sec, {
        stream: options.stream.value,
      });
      if (requestId === statsRequestId) stats.value = result;
    } catch (error: any) {
      if (requestId !== statsRequestId) return;
      statsError.value =
        error?.response?.data?.message ||
        error?.message ||
        t("admin.hostTraffic.loadFailed");
    } finally {
      if (requestId === statsRequestId) isStatsLoading.value = false;
    }
  };

  watch(
    () => [options.sample.value, options.timestamp.value] as const,
    ([sample, timestamp]) => {
      if (!sample) {
        realtimeInBps.value = null;
        realtimeOutBps.value = null;
        lastRealtimeSample = null;
        return;
      }

      const now = Number(timestamp ?? Date.now());
      const totalIn = Number(sample.total_in ?? 0);
      const totalOut = Number(sample.total_out ?? 0);
      if (!Number.isFinite(totalIn) || !Number.isFinite(totalOut)) return;

      if (lastRealtimeSample && Number.isFinite(now)) {
        const dt = Math.max(1, (now - lastRealtimeSample.at) / 1000);
        realtimeInBps.value =
          Math.max(0, totalIn - lastRealtimeSample.totalIn) / dt;
        realtimeOutBps.value =
          Math.max(0, totalOut - lastRealtimeSample.totalOut) / dt;
      }

      lastRealtimeSample = {
        at: Number.isFinite(now) ? now : Date.now(),
        totalIn,
        totalOut,
      };
    },
    { immediate: true },
  );

  watch(
    () => [options.active.value, rangeKey.value, options.stream.value] as const,
    ([active]) => {
      if (active) void loadStats();
    },
    { immediate: true },
  );

  return {
    compactInText,
    compactOutText,
    formatBps,
    formatBytes,
    hasCompactTraffic,
    hasRealtimeInTraffic,
    hasRealtimeOutTraffic,
    isStatsLoading,
    ranges,
    rangeKey,
    rangeText,
    realtimeInText,
    realtimeOutText,
    stats,
    statsError,
    trafficSeries,
  };
};
