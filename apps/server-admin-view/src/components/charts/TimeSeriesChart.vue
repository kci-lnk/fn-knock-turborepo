<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, shallowRef, watch } from "vue";
import { useI18n } from "vue-i18n";
import uPlot from "uplot";
import "uplot/dist/uPlot.min.css";

export type TimeSeriesPoint = readonly [
  number | string | Date,
  number | null | undefined,
];

export type TimeSeriesChartSeries = {
  name: string;
  data: readonly TimeSeriesPoint[];
  color: string;
  fill?: string;
  width?: number;
};

type TooltipItem = {
  name: string;
  color: string;
  value: string;
};

type LegendItem = {
  name: string;
  color: string;
};

const props = withDefaults(
  defineProps<{
    series: readonly TimeSeriesChartSeries[];
    valueFormatter?: (value: number) => string;
    timeFormatter?: (value: number) => string;
    emptyText?: string;
    minHeight?: number;
    yMin?: number;
    showLegend?: boolean;
  }>(),
  {
    valueFormatter: undefined,
    timeFormatter: undefined,
    emptyText: "",
    minHeight: 120,
    yMin: undefined,
    showLegend: true,
  },
);

const root = ref<HTMLElement | null>(null);
const plot = shallowRef<uPlot | null>(null);
const tooltip = ref<{
  visible: boolean;
  left: number;
  top: number;
  time: string;
  items: TooltipItem[];
}>({
  visible: false,
  left: 0,
  top: 0,
  time: "",
  items: [],
});

const { locale } = useI18n();
let resizeObserver: ResizeObserver | null = null;
let themeObserver: MutationObserver | null = null;

const toTimestampMs = (value: TimeSeriesPoint[0]): number | null => {
  if (value instanceof Date) {
    const time = value.getTime();
    return Number.isFinite(time) ? time : null;
  }

  if (typeof value === "number") {
    if (!Number.isFinite(value)) return null;
    return Math.abs(value) < 100_000_000_000 ? value * 1000 : value;
  }

  const parsed = new Date(value).getTime();
  return Number.isFinite(parsed) ? parsed : null;
};

const normalizeNumber = (value: number | null | undefined): number | null => {
  if (value === null || value === undefined) return null;
  const normalized = Number(value);
  return Number.isFinite(normalized) ? normalized : null;
};

const alignedData = computed(() => {
  const xSet = new Set<number>();
  const seriesMaps = props.series.map((item) => {
    const points = new Map<number, number | null>();
    for (const point of item.data) {
      const x = toTimestampMs(point[0]);
      if (x === null) continue;
      xSet.add(x);
      points.set(x, normalizeNumber(point[1]));
    }
    return points;
  });

  const xValues = Array.from(xSet).sort((left, right) => left - right);
  const values = seriesMaps.map((points) =>
    xValues.map((x) => points.get(x) ?? null),
  );

  return [xValues, ...values] as uPlot.AlignedData;
});

const hasRenderableData = computed(() =>
  alignedData.value
    .slice(1)
    .some((values) => values.some((value) => value !== null && value !== undefined)),
);

const legendItems = computed<LegendItem[]>(() =>
  props.series
    .map((item, index) => ({
      name: item.name.trim() || `Series ${index + 1}`,
      color: item.color,
    }))
    .filter((item) => item.name),
);

const shouldShowLegend = computed(
  () => props.showLegend && legendItems.value.length > 1,
);

const formatValue = (value: number | null | undefined) => {
  if (value === null || value === undefined || !Number.isFinite(value)) {
    return "-";
  }
  return props.valueFormatter?.(value) ?? new Intl.NumberFormat(String(locale.value)).format(value);
};

const formatTime = (value: number, compact = false) => {
  if (props.timeFormatter) return props.timeFormatter(value);

  const date = new Date(value);
  if (!Number.isFinite(date.getTime())) return "";

  return new Intl.DateTimeFormat(String(locale.value), {
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
    ...(compact ? {} : { second: "2-digit" }),
  }).format(date);
};

const isDarkMode = () =>
  typeof document !== "undefined" &&
  document.documentElement.classList.contains("dark");

const getChartColors = () => {
  const dark = isDarkMode();
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
    height: Math.max(props.minHeight, Math.floor(rect.height || el.clientHeight || props.minHeight)),
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
    value: formatValue(chart.data[seriesIndex + 1]?.[idx] as number | null | undefined),
  }));

  const size = getChartSize();
  const left = Math.min(Math.max(Number(chart.cursor.left ?? 0) + 14, 8), Math.max(8, size.width - 180));
  const top = Math.min(Math.max(Number(chart.cursor.top ?? 0) + 14, 8), Math.max(8, size.height - 96));

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
          if (!Number.isFinite(min) || !Number.isFinite(max)) return [floor, 1];
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
        values: (_chart, splits) => splits.map((value) => formatTime(value, true)),
        size: 34,
      },
      {
        stroke: colors.axis,
        grid: { stroke: colors.grid, width: 1 },
        ticks: { show: false },
        values: (_chart, splits) => splits.map((value) => formatValue(value)),
        size: 60,
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
  () => {
    void renderPlot();
  },
  { deep: true },
);

onMounted(() => {
  void renderPlot();

  if (root.value) {
    resizeObserver = new ResizeObserver(resizePlot);
    resizeObserver.observe(root.value);
  }

  if (typeof document !== "undefined") {
    themeObserver = new MutationObserver(() => {
      void renderPlot();
    });
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
</script>

<template>
  <div class="time-series-chart flex h-full w-full min-w-0 flex-col overflow-hidden">
    <div
      v-if="shouldShowLegend"
      class="flex shrink-0 flex-wrap items-center justify-end gap-x-3 gap-y-1 px-1 pb-1 text-[11px] leading-4 text-muted-foreground"
    >
      <span
        v-for="item in legendItems"
        :key="item.name"
        class="inline-flex min-w-0 max-w-40 items-center gap-1.5"
        :title="item.name"
      >
        <span
          class="h-2 w-2 shrink-0 rounded-full"
          :style="{ backgroundColor: item.color }"
        ></span>
        <span class="truncate">{{ item.name }}</span>
      </span>
    </div>
    <div class="relative min-h-0 w-full flex-1">
      <div ref="root" class="h-full w-full min-w-0"></div>
      <div
        v-if="!hasRenderableData && emptyText"
        class="absolute inset-0 flex items-center justify-center text-xs text-muted-foreground"
      >
        {{ emptyText }}
      </div>
      <div
        v-if="tooltip.visible"
        class="pointer-events-none absolute z-10 min-w-36 rounded-md border border-white/10 bg-black/85 px-3 py-2 text-xs text-white shadow-lg"
        :style="{ left: `${tooltip.left}px`, top: `${tooltip.top}px` }"
      >
        <div class="mb-1 font-medium text-white/85">{{ tooltip.time }}</div>
        <div
          v-for="item in tooltip.items"
          :key="item.name"
          class="flex items-center justify-between gap-4"
        >
          <span class="inline-flex min-w-0 items-center gap-1.5">
            <span
              class="h-2 w-2 shrink-0 rounded-full"
              :style="{ backgroundColor: item.color }"
            ></span>
            <span class="truncate">{{ item.name }}</span>
          </span>
          <span class="shrink-0 font-medium">{{ item.value }}</span>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.time-series-chart :deep(.uplot) {
  height: 100%;
  width: 100%;
}

.time-series-chart :deep(.u-over),
.time-series-chart :deep(.u-under) {
  max-width: 100%;
}

.time-series-chart :deep(.u-cursor-x) {
  border-right-color: rgba(115, 115, 115, 0.55);
}

:global(.dark) .time-series-chart :deep(.u-cursor-x) {
  border-right-color: rgba(212, 212, 212, 0.45);
}
</style>
