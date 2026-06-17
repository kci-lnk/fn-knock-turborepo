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
  { key: '15m', label: '15m', sec: 15 * 60 },
  { key: '1h', label: '1h', sec: 60 * 60 },
  { key: '6h', label: '6h', sec: 6 * 60 * 60 },
  { key: '1d', label: '24h', sec: 24 * 60 * 60 },
  { key: '7d', label: '7d', sec: 7 * 24 * 60 * 60 },
];

interface UseThreatOverviewOptions {
  defaultRangeKey: string;
  ranges: ThreatRange[];
  seriesKey: 'failedLogins' | 'blockedScanners';
  fetchOverview: (rangeSec: number) => Promise<ThreatOverviewModel>;
  onError: (error: unknown) => void;
  formatRangeText?: (seconds: number) => string;
  numberLocale?: string | (() => string);
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
    if (options.formatRangeText) return options.formatRangeText(sec);
    if (sec < 3600) return `${Math.round(sec / 60)}m`;
    if (sec < 24 * 3600) return `${Math.round(sec / 3600)}h`;
    return `${Math.round(sec / 86400)}d`;
  });

  const perHour = computed(() => {
    const total = threatOverview.value?.totals[options.seriesKey] ?? 0;
    const sec = threatOverview.value?.rangeSec ?? activeRange.value.sec;
    const hours = sec / 3600;
    if (!Number.isFinite(hours) || hours <= 0) return 0;
    return total / hours;
  });

  const resolveNumberLocale = () =>
    typeof options.numberLocale === 'function'
      ? options.numberLocale()
      : (options.numberLocale ?? 'zh-CN');

  const formatNumber = (value: number | null | undefined) => {
    const normalized = Number(value ?? 0);
    if (!Number.isFinite(normalized)) return '-';
    return new Intl.NumberFormat(resolveNumberLocale()).format(
      Math.round(normalized),
    );
  };

  const formatRate = (value: number) =>
    new Intl.NumberFormat(resolveNumberLocale(), {
      maximumFractionDigits: 1,
    }).format(value);

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
    fetchThreatOverview,
  };
}
