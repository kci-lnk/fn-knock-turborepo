import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
} from "vue";
import { useI18n } from "vue-i18n";
import uPlot from "uplot";
import {
  alignTimeSeriesData,
  buildTimeSeriesLegendItems,
  getTimeSeriesYAxisSize,
  hasRenderableTimeSeriesData,
  type TimeSeriesChartSeries,
} from "./timeSeriesChartModel";

export interface TimeSeriesChartProps {
  series: readonly TimeSeriesChartSeries[];
  valueFormatter?: (value: number) => string;
  timeFormatter?: (value: number) => string;
  emptyText?: string;
  minHeight?: number;
  yMin?: number;
  showLegend?: boolean;
}

type ResolvedTimeSeriesChartProps = TimeSeriesChartProps & {
  emptyText: string;
  minHeight: number;
  showLegend: boolean;
};

type TooltipItem = {
  name: string;
  color: string;
  value: string;
};

const DAY_MS = 24 * 60 * 60 * 1000;
const LONG_RANGE_MS = DAY_MS * 2;
const X_AXIS_LABEL_GAP = 8;

export function useTimeSeriesChart(props: ResolvedTimeSeriesChartProps) {
  const root = ref<HTMLElement | null>(null);
  const plot = shallowRef<uPlot | null>(null);
  const tooltip = ref<{
    visible: boolean;
    left: number;
    top: number;
    time: string;
    items: TooltipItem[];
  }>({ visible: false, left: 0, top: 0, time: "", items: [] });
  const { locale } = useI18n();
  let resizeObserver: ResizeObserver | null = null;
  let themeObserver: MutationObserver | null = null;

  const alignedData = computed(() => alignTimeSeriesData(props.series));
  const hasRenderableData = computed(() =>
    hasRenderableTimeSeriesData(alignedData.value),
  );
  const legendItems = computed(() =>
    buildTimeSeriesLegendItems(props.series),
  );
  const shouldShowLegend = computed(
    () => props.showLegend && legendItems.value.length > 1,
  );

  const formatValue = (value: number | null | undefined) => {
    if (value === null || value === undefined || !Number.isFinite(value)) {
      return "-";
    }
    return (
      props.valueFormatter?.(value) ??
      new Intl.NumberFormat(String(locale.value)).format(value)
    );
  };

  const formatTime = (value: number, compact = false) => {
    if (props.timeFormatter) return props.timeFormatter(value);
    const date = new Date(value);
    if (!Number.isFinite(date.getTime())) return "";
    return new Intl.DateTimeFormat(String(locale.value), {
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      ...(compact ? {} : { second: "2-digit" }),
    }).format(date);
  };

  const formatDay = (date: Date) =>
    new Intl.DateTimeFormat(String(locale.value), {
      day: "2-digit",
    }).format(date);
  const formatClock = (date: Date) =>
    new Intl.DateTimeFormat(String(locale.value), {
      hour: "2-digit",
      minute: "2-digit",
    }).format(date);
  const getDateKey = (value: number) => {
    const date = new Date(value);
    return `${date.getFullYear()}-${date.getMonth()}-${date.getDate()}`;
  };
  const estimateAxisLabelWidth = (label: string) =>
    Math.max(
      ...label.split("\n").map((line) =>
        Array.from(line).reduce(
          (width, char) => width + (char.charCodeAt(0) > 255 ? 12 : 7),
          0,
        ),
      ),
    );

  const thinXAxisLabels = (
    chart: uPlot,
    splits: number[],
    labels: string[],
  ) => {
    const visibleLabels = Array.from({ length: labels.length }, () => "");
    let lastRight = Number.NEGATIVE_INFINITY;
    labels.forEach((label, index) => {
      if (!label) return;
      const split = splits[index];
      if (split === undefined) return;
      const x = chart.valToPos(split, "x");
      if (!Number.isFinite(x)) return;
      const halfWidth = estimateAxisLabelWidth(label) / 2;
      const left = x - halfWidth;
      const right = x + halfWidth;
      if (left >= lastRight + X_AXIS_LABEL_GAP) {
        visibleLabels[index] = label;
        lastRight = right;
      }
    });
    return visibleLabels;
  };

  const formatXAxisValues = (chart: uPlot, splits: number[]) => {
    const xScale = chart.scales.x;
    const firstSplit = splits[0] ?? 0;
    const lastSplit = splits[splits.length - 1] ?? firstSplit;
    const rangeMs =
      xScale && typeof xScale.min === "number" && typeof xScale.max === "number"
        ? xScale.max - xScale.min
        : lastSplit - firstSplit;
    if (!Number.isFinite(rangeMs)) {
      return splits.map((value) => formatTime(value, true));
    }
    if (props.timeFormatter) {
      return thinXAxisLabels(
        chart,
        splits,
        splits.map((value) => props.timeFormatter?.(value) ?? ""),
      );
    }
    if (rangeMs >= LONG_RANGE_MS) {
      let previousDateKey = "";
      const labels = splits.map((value) => {
        const date = new Date(value);
        if (!Number.isFinite(date.getTime())) return "";
        const dateKey = getDateKey(value);
        if (dateKey === previousDateKey) return "";
        previousDateKey = dateKey;
        return formatDay(date);
      });
      return thinXAxisLabels(chart, splits, labels);
    }
    return thinXAxisLabels(
      chart,
      splits,
      splits.map((value) => {
        const date = new Date(value);
        if (!Number.isFinite(date.getTime())) return "";
        return rangeMs >= DAY_MS
          ? `${formatDay(date)} ${formatClock(date)}`
          : formatClock(date);
      }),
    );
  };

  const getChartColors = () => {
    const dark =
      typeof document !== "undefined" &&
      document.documentElement.classList.contains("dark");
    return {
      axis: dark ? "#a3a3a3" : "#737373",
      grid: dark ? "rgba(245,245,245,0.12)" : "#f5f5f5",
    };
  };

  const getChartSize = () => {
    const el = root.value;
    if (!el) return { width: 320, height: props.minHeight };
    const rect = el.getBoundingClientRect();
    return {
      width: Math.max(160, Math.floor(rect.width || el.clientWidth || 320)),
      height: Math.max(
        props.minHeight,
        Math.floor(rect.height || el.clientHeight || props.minHeight),
      ),
    };
  };

  const updateTooltip = (chart: uPlot) => {
    const idx = chart.cursor.idx;
    if (idx === null || idx === undefined || idx < 0) {
      tooltip.value.visible = false;
      return;
    }
    const rawTime = chart.data[0]?.[idx];
    if (rawTime === null || rawTime === undefined) {
      tooltip.value.visible = false;
      return;
    }
    const items = props.series.map((item, seriesIndex) => ({
      name: item.name,
      color: item.color,
      value: formatValue(
        chart.data[seriesIndex + 1]?.[idx] as number | null | undefined,
      ),
    }));
    const size = getChartSize();
    const left = Math.min(
      Math.max(Number(chart.cursor.left ?? 0) + 14, 8),
      Math.max(8, size.width - 180),
    );
    const top = Math.min(
      Math.max(Number(chart.cursor.top ?? 0) + 14, 8),
      Math.max(8, size.height - 96),
    );
    tooltip.value = {
      visible: true,
      left,
      top,
      time: formatTime(Number(rawTime)),
      items,
    };
  };

  const destroyPlot = () => {
    tooltip.value.visible = false;
    plot.value?.destroy();
    plot.value = null;
  };

  const createOptions = (): uPlot.Options => {
    const size = getChartSize();
    const colors = getChartColors();
    return {
      width: size.width,
      height: size.height,
      ms: 1,
      padding: [8, 8, 0, 0],
      legend: { show: false },
      cursor: {
        drag: { x: false, y: false },
        points: { size: 6 },
        x: true,
        y: false,
      },
      scales: {
        x: { time: true },
        y: {
          range: (_chart, min, max) => {
            const floor = props.yMin ?? 0;
            if (!Number.isFinite(min) || !Number.isFinite(max)) {
              return [floor, 1];
            }
            if (min === max) return [floor, Math.max(1, max * 1.2 || 1)];
            return [Math.min(floor, min), max * 1.08];
          },
        },
      },
      axes: [
        {
          stroke: colors.axis,
          grid: { stroke: colors.grid, width: 1 },
          ticks: { show: false },
          values: formatXAxisValues,
          size: 34,
        },
        {
          stroke: colors.axis,
          grid: { stroke: colors.grid, width: 1 },
          ticks: { show: false },
          values: (_chart, splits) =>
            splits.map((value) => formatValue(value)),
          size: (_chart, values) =>
            getTimeSeriesYAxisSize(values, estimateAxisLabelWidth),
        },
      ],
      series: [
        {},
        ...props.series.map((item) => ({
          label: item.name,
          stroke: item.color,
          fill: item.fill ?? `${item.color}14`,
          width: item.width ?? 2,
          spanGaps: true,
          points: { show: false },
        })),
      ],
      hooks: {
        setCursor: [updateTooltip],
        ready: [updateTooltip],
      },
    };
  };

  const renderPlot = async () => {
    await nextTick();
    const el = root.value;
    if (!el || !hasRenderableData.value) {
      destroyPlot();
      return;
    }
    destroyPlot();
    plot.value = new uPlot(createOptions(), alignedData.value, el);
  };
  const resizePlot = () => {
    if (!plot.value || !root.value) return;
    plot.value.setSize(getChartSize());
  };

  watch(
    () =>
      [
        props.series,
        locale.value,
        props.valueFormatter,
        props.timeFormatter,
        props.yMin,
        props.showLegend,
      ] as const,
    () => void renderPlot(),
    { deep: true },
  );

  onMounted(() => {
    void renderPlot();
    if (root.value) {
      resizeObserver = new ResizeObserver(resizePlot);
      resizeObserver.observe(root.value);
    }
    if (typeof document !== "undefined") {
      themeObserver = new MutationObserver(() => void renderPlot());
      themeObserver.observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["class"],
      });
    }
  });

  onBeforeUnmount(() => {
    resizeObserver?.disconnect();
    resizeObserver = null;
    themeObserver?.disconnect();
    themeObserver = null;
    destroyPlot();
  });

  return {
    hasRenderableData,
    legendItems,
    root,
    shouldShowLegend,
    tooltip,
  };
}
