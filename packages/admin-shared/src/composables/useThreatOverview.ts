import { computed, ref, watch } from 'vue';

type ThreatSeries = Array<[number, number]>;

export type ThreatOverviewModel = {
  rangeSec: number;
  totals: {
    failedLogins: number;
    blockedScanners: number;
  };
  series: {
    failedLogins: ThreatSeries;
    blockedScanners: ThreatSeries;
  };
};

export type ThreatRange = {
  key: string;
  label: string;
  sec: number;
};

export const DEFAULT_THREAT_RANGES: ThreatRange[] = [
  { key: '15m', label: '15分钟', sec: 15 * 60 },
  { key: '1h', label: '1小时', sec: 60 * 60 },
  { key: '6h', label: '6小时', sec: 6 * 60 * 60 },
  { key: '1d', label: '24小时', sec: 24 * 60 * 60 },
  { key: '7d', label: '7天', sec: 7 * 24 * 60 * 60 },
];

interface UseThreatOverviewOptions {
  defaultRangeKey: string;
  ranges: ThreatRange[];
  seriesKey: 'failedLogins' | 'blockedScanners';
  seriesName: string;
  lineColor: string;
  areaStartColor: string;
  areaEndColor: string;
  fetchOverview: (rangeSec: number) => Promise<ThreatOverviewModel>;
  onError: (error: unknown) => void;
}

export function useThreatOverview(options: UseThreatOverviewOptions) {
  const fallbackRange: ThreatRange = {
    key: options.defaultRangeKey,
    label: options.defaultRangeKey,
    sec: 3600,
  };
  const rangeKey = ref(options.defaultRangeKey);
  const threatOverview = ref<ThreatOverviewModel | null>(null);
  const isThreatLoading = ref(false);

  const activeRange = computed(
    () =>
      options.ranges.find((range) => range.key === rangeKey.value) ??
      options.ranges[0] ??
      fallbackRange,
  );

  const titleRangeText = computed(() => {
    const sec = threatOverview.value?.rangeSec ?? activeRange.value.sec;
    if (sec < 3600) return `${Math.round(sec / 60)} 分钟`;
    if (sec < 24 * 3600) return `${Math.round(sec / 3600)} 小时`;
    return `${Math.round(sec / 86400)} 天`;
  });

  const perHour = computed(() => {
    const total = threatOverview.value?.totals[options.seriesKey] ?? 0;
    const sec = threatOverview.value?.rangeSec ?? activeRange.value.sec;
    const hours = sec / 3600;
    if (!Number.isFinite(hours) || hours <= 0) return 0;
    return total / hours;
  });

  const formatNumber = (value: number | null | undefined) => {
    const normalized = Number(value ?? 0);
    if (!Number.isFinite(normalized)) return '-';
    return new Intl.NumberFormat('zh-CN').format(Math.round(normalized));
  };

  const formatRate = (value: number) =>
    new Intl.NumberFormat('zh-CN', { maximumFractionDigits: 1 }).format(value);

  const trendOption = computed(() => ({
    color: [options.lineColor],
    animationDuration: 420,
    tooltip: {
      trigger: 'axis',
      axisPointer: { type: 'cross' },
      backgroundColor: 'rgba(10, 10, 10, 0.9)',
      borderColor: 'rgba(255,255,255,0.08)',
      textStyle: { color: '#fff' },
    },
    grid: { left: 18, right: 18, top: 36, bottom: 24, containLabel: true },
    xAxis: {
      type: 'time',
      boundaryGap: false,
      axisLabel: { color: 'rgba(120,120,120,0.9)' },
      axisLine: { lineStyle: { color: 'rgba(120,120,120,0.25)' } },
      splitLine: { show: false },
    },
    yAxis: {
      type: 'value',
      minInterval: 1,
      axisLabel: { color: 'rgba(120,120,120,0.9)' },
      axisLine: { show: false },
      splitLine: { lineStyle: { color: 'rgba(120,120,120,0.18)' } },
    },
    series: [
      {
        name: options.seriesName,
        type: 'line',
        smooth: true,
        symbol: 'none',
        lineStyle: { width: 2 },
        areaStyle: {
          opacity: 1,
          color: {
            type: 'linear',
            x: 0,
            y: 0,
            x2: 0,
            y2: 1,
            colorStops: [
              { offset: 0, color: options.areaStartColor },
              { offset: 1, color: options.areaEndColor },
            ],
          },
        },
        data: threatOverview.value?.series[options.seriesKey] ?? [],
      },
    ],
  }));

  const fetchThreatOverview = async () => {
    isThreatLoading.value = true;
    try {
      threatOverview.value = await options.fetchOverview(activeRange.value.sec);
    } catch (error) {
      options.onError(error);
    } finally {
      isThreatLoading.value = false;
    }
  };

  watch(rangeKey, () => {
    fetchThreatOverview();
  });

  return {
    rangeKey,
    threatOverview,
    isThreatLoading,
    titleRangeText,
    perHour,
    formatNumber,
    formatRate,
    trendOption,
    fetchThreatOverview,
  };
}
