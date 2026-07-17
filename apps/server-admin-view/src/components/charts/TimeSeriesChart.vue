<script setup lang="ts">
import "uplot/dist/uPlot.min.css";
import {
  useTimeSeriesChart,
  type TimeSeriesChartProps,
} from "./useTimeSeriesChart";

export type {
  TimeSeriesChartSeries,
  TimeSeriesPoint,
} from "./timeSeriesChartModel";

const props = withDefaults(defineProps<TimeSeriesChartProps>(), {
  valueFormatter: undefined,
  timeFormatter: undefined,
  emptyText: "",
  minHeight: 120,
  yMin: undefined,
  showLegend: true,
});

const {
  hasRenderableData,
  legendItems,
  root,
  shouldShowLegend,
  tooltip,
} = useTimeSeriesChart(props);

// Vue assigns this string template ref at runtime.
void root;
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
