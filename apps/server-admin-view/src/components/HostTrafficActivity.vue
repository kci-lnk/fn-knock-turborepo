<template>
  <Popover :open="open" @update:open="handleOpenChange">
    <PopoverAnchor as-child>
      <button
        type="button"
        class="inline-flex min-h-6 max-w-full flex-wrap items-center gap-x-2 gap-y-1 px-1.5 text-left text-xs leading-none transition-colors hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :class="{ 'border-primary/30 bg-primary/5': open || dialogOpen }"
        :aria-label="
          t('admin.hostTraffic.detailsAria', {
            title: displayTitle,
            host,
          })
        "
        @pointerdown="handleTriggerPointerDown"
        @pointerenter="handleTriggerPointerEnter"
        @pointerleave="handleTriggerPointerLeave"
        @focus="handleTriggerFocus"
        @blur="handleTriggerBlur"
        @click.prevent="handleTriggerClick"
      >
        <span
          v-if="hasRealtimeInTraffic"
          class="inline-flex items-center gap-1"
        >
          <ArrowDownLeft class="h-3 w-3 shrink-0 text-emerald-700" />
          <span>{{ compactInText }}</span>
        </span>
        <span
          v-if="hasRealtimeOutTraffic"
          class="inline-flex items-center gap-1"
        >
          <ArrowUpRight class="h-3 w-3 shrink-0 text-blue-700" />
          <span>{{ compactOutText }}</span>
        </span>
        <span v-if="!hasCompactTraffic">{{ t("admin.hostTraffic.view") }}</span>
      </button>
    </PopoverAnchor>

    <PopoverContent
      v-if="!isTouchInteraction"
      side="left"
      align="center"
      class="w-[28rem] max-w-[92vw] rounded-md p-0 text-left"
      @pointerenter="handleContentPointerEnter"
      @pointerleave="handleContentPointerLeave"
    >
      <div class="border-b px-4 py-3">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <div class="truncate text-sm font-semibold" :title="displayTitle">
              {{ displayTitle }}
            </div>
            <div
              class="mt-1 break-all text-xs font-medium text-muted-foreground"
              :title="host"
            >
              {{ host }}
            </div>
            <div class="mt-1 text-xs text-muted-foreground">
              {{ sampleStatusText }}
            </div>
          </div>
          <Button
            variant="outline"
            size="sm"
            class="h-7 shrink-0 px-2 text-xs"
            @click.stop.prevent="openActiveIpDialog"
          >
            <Network class="h-3.5 w-3.5" />
            {{ activeIpButtonText }}
          </Button>
        </div>
      </div>

      <div class="space-y-4 p-4">
        <div class="grid gap-2 sm:grid-cols-2">
          <div class="rounded-md border bg-muted/20 px-3 py-2.5">
            <div
              class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
            >
              <ArrowDownLeft class="h-3.5 w-3.5 text-emerald-700" />
              {{ t("admin.hostTraffic.realtimeIn") }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ realtimeInText }}
            </div>
          </div>
          <div class="rounded-md border bg-muted/20 px-3 py-2.5">
            <div
              class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
            >
              <ArrowUpRight class="h-3.5 w-3.5 text-blue-700" />
              {{ t("admin.hostTraffic.realtimeOut") }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ realtimeOutText }}
            </div>
          </div>
        </div>

        <Tabs v-model="rangeKey" class="w-full">
          <TabsList class="grid w-full grid-cols-5">
            <TabsTrigger
              v-for="range in ranges"
              :key="range.key"
              :value="range.key"
              class="px-2 text-xs"
            >
              {{ range.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>

        <div class="grid gap-2 sm:grid-cols-2">
          <div class="rounded-md border px-3 py-2.5">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.hostTraffic.cumulativeIn", { range: rangeText }) }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ formatBytes(stats?.totals.inBytes) }}
            </div>
          </div>
          <div class="rounded-md border px-3 py-2.5">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.hostTraffic.cumulativeOut", { range: rangeText }) }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ formatBytes(stats?.totals.outBytes) }}
            </div>
          </div>
        </div>

        <div class="h-[140px] w-full overflow-hidden rounded-md border">
          <div
            v-if="isStatsLoading"
            class="flex h-full items-center justify-center p-4"
          >
            <Skeleton class="h-full w-full rounded-md" />
          </div>
          <div
            v-else-if="statsError"
            class="flex h-full items-center justify-center px-4 text-center text-xs text-muted-foreground"
          >
            {{ statsError }}
          </div>
          <TimeSeriesChart
            v-else
            :series="trafficSeries"
            :value-formatter="formatBps"
            class="h-full w-full"
          />
        </div>
      </div>
    </PopoverContent>
  </Popover>

  <Dialog :open="dialogOpen" @update:open="handleDialogOpenChange">
    <DialogContent
      class="max-h-[88vh] overflow-y-auto p-0 text-left sm:max-w-[28rem]"
    >
      <DialogHeader class="border-b px-4 py-3 pr-10 text-left">
        <div class="flex items-center justify-between gap-3">
          <div class="min-w-0">
            <DialogTitle class="truncate text-base" :title="displayTitle">
              {{ displayTitle }}
            </DialogTitle>
            <DialogDescription class="space-y-1 text-left">
              <span class="block break-all font-medium">{{ host }}</span>
              <span class="block text-xs">{{ sampleStatusText }}</span>
            </DialogDescription>
          </div>
          <Button
            variant="outline"
            size="sm"
            class="h-7 shrink-0 px-2 text-xs"
            @click.stop.prevent="openActiveIpDialog"
          >
            <Network class="h-3.5 w-3.5" />
            {{ activeIpButtonText }}
          </Button>
        </div>
      </DialogHeader>

      <div class="space-y-4 p-4">
        <div class="grid gap-2 sm:grid-cols-2">
          <div class="rounded-md border bg-muted/20 px-3 py-2.5">
            <div
              class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
            >
              <ArrowDownLeft class="h-3.5 w-3.5 text-emerald-700" />
              {{ t("admin.hostTraffic.realtimeIn") }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ realtimeInText }}
            </div>
          </div>
          <div class="rounded-md border bg-muted/20 px-3 py-2.5">
            <div
              class="flex items-center gap-1.5 text-xs font-medium text-muted-foreground"
            >
              <ArrowUpRight class="h-3.5 w-3.5 text-blue-700" />
              {{ t("admin.hostTraffic.realtimeOut") }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ realtimeOutText }}
            </div>
          </div>
        </div>

        <Tabs v-model="rangeKey" class="w-full">
          <TabsList class="grid w-full grid-cols-5">
            <TabsTrigger
              v-for="range in ranges"
              :key="range.key"
              :value="range.key"
              class="px-2 text-xs"
            >
              {{ range.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>

        <div class="grid gap-2 sm:grid-cols-2">
          <div class="rounded-md border px-3 py-2.5">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.hostTraffic.cumulativeIn", { range: rangeText }) }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ formatBytes(stats?.totals.inBytes) }}
            </div>
          </div>
          <div class="rounded-md border px-3 py-2.5">
            <div class="text-xs text-muted-foreground">
              {{ t("admin.hostTraffic.cumulativeOut", { range: rangeText }) }}
            </div>
            <div class="mt-1 text-base font-semibold">
              {{ formatBytes(stats?.totals.outBytes) }}
            </div>
          </div>
        </div>

        <div class="h-[140px] w-full overflow-hidden rounded-md border">
          <div
            v-if="isStatsLoading"
            class="flex h-full items-center justify-center p-4"
          >
            <Skeleton class="h-full w-full rounded-md" />
          </div>
          <div
            v-else-if="statsError"
            class="flex h-full items-center justify-center px-4 text-center text-xs text-muted-foreground"
          >
            {{ statsError }}
          </div>
          <TimeSeriesChart
            v-else
            :series="trafficSeries"
            :value-formatter="formatBps"
            class="h-full w-full"
          />
        </div>
      </div>
    </DialogContent>
  </Dialog>

  <HostActiveIpDialog
    v-model:open="activeIpDialogOpen"
    :title="displayTitle"
    :host="host"
    :items="activeIpItems"
    :loading="activeIpLoading"
    :error="activeIpError"
    :updated-at="activeIpUpdatedAt"
    :window-seconds="activeIpWindowSeconds"
    @refresh="refreshActiveIps"
  />
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { ArrowDownLeft, ArrowUpRight, Network } from "lucide-vue-next";
import {
  Popover,
  PopoverAnchor,
  PopoverContent,
} from "@/components/ui/popover";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import HostActiveIpDialog from "@/components/host-traffic/HostActiveIpDialog.vue";
import TimeSeriesChart, {
  type TimeSeriesChartSeries,
} from "@/components/charts/TimeSeriesChart.vue";
import { useHostActiveIps } from "@/composables/useHostActiveIps";
import { DashboardAPI } from "../lib/api";
import type { DashboardStats, HostTrafficStats } from "../types";

const rangeDefs = [
  { key: "15m", sec: 15 * 60 },
  { key: "1h", sec: 60 * 60 },
  { key: "6h", sec: 6 * 60 * 60 },
  { key: "1d", sec: 24 * 60 * 60 },
  { key: "7d", sec: 7 * 24 * 60 * 60 },
] as const;

type RangeKey = (typeof rangeDefs)[number]["key"];

const props = withDefaults(
  defineProps<{
    host: string;
    title?: string | null;
    sample?: HostTrafficStats | null;
    timestamp?: number | null;
  }>(),
  {
    title: "",
    sample: null,
    timestamp: null,
  },
);

const { t } = useI18n();
const open = ref(false);
const dialogOpen = ref(false);
const activeIpDialogOpen = ref(false);
const isTouchInteraction = ref(false);
const lastTriggerPointerType = ref<string | null>(null);
const suppressNextFocusOpen = ref(false);
const rangeKey = ref<RangeKey>("1h");
const stats = ref<DashboardStats | null>(null);
const isStatsLoading = ref(false);
const statsError = ref("");
const realtimeInBps = ref<number | null>(null);
const realtimeOutBps = ref<number | null>(null);
let closeTimer: number | null = null;
let statsRequestId = 0;
let lastRealtimeSample: {
  at: number;
  totalIn: number;
  totalOut: number;
} | null = null;
let interactionMediaQuery: MediaQueryList | null = null;

const {
  displayItems: activeIpItems,
  loading: activeIpLoading,
  error: activeIpError,
  updatedAt: activeIpUpdatedAt,
  windowSeconds: activeIpWindowSeconds,
  refresh: refreshActiveIps,
} = useHostActiveIps(computed(() => props.host), activeIpDialogOpen);

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

const hasRealtimeSample = computed(() => Boolean(props.sample));
const displayTitle = computed(
  () => props.title?.trim() || t("admin.hostTraffic.unknownTitle"),
);
const sampleStatusText = computed(() =>
  hasRealtimeSample.value
    ? t("admin.hostTraffic.sampling")
    : t("admin.hostTraffic.waitingSample"),
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

const formatBps = (bps: number | null | undefined) => {
  if (bps === null || bps === undefined) return "-";
  return `${formatBytes(bps)} /s`;
};

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
const activeIpButtonText = computed(() => {
  const count = Number(
    props.sample?.active_ip_count ?? activeIpItems.value.length,
  );
  return count > 0
    ? t("admin.hostTraffic.activeIpWithCount", { count })
    : t("admin.hostTraffic.activeIp");
});

const rangeText = computed(() => {
  const sec = stats.value?.rangeSec ?? activeRange.value.sec;
  return formatPlainRangeText(sec);
});

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

const clearCloseTimer = () => {
  if (closeTimer !== null) {
    window.clearTimeout(closeTimer);
    closeTimer = null;
  }
};

const updateInteractionMode = () => {
  const nextIsTouchInteraction = Boolean(interactionMediaQuery?.matches);
  isTouchInteraction.value = nextIsTouchInteraction;

  if (nextIsTouchInteraction) {
    open.value = false;
    clearCloseTimer();
    return;
  }

  dialogOpen.value = false;
};

function openPanel() {
  if (isTouchInteraction.value) return;

  clearCloseTimer();
  open.value = true;
}

function scheduleClosePanel() {
  if (isTouchInteraction.value) return;

  clearCloseTimer();
  closeTimer = window.setTimeout(() => {
    open.value = false;
    closeTimer = null;
  }, 140);
}

function handleOpenChange(nextOpen: boolean) {
  clearCloseTimer();
  if (isTouchInteraction.value) {
    open.value = false;
    return;
  }

  open.value = nextOpen;
}

function isMousePointer(event: PointerEvent) {
  return event.pointerType === "mouse";
}

function handleTriggerPointerDown(event: PointerEvent) {
  lastTriggerPointerType.value = event.pointerType;

  if (!isMousePointer(event)) {
    suppressNextFocusOpen.value = true;
    clearCloseTimer();
    open.value = false;
  }
}

function handleTriggerPointerEnter(event: PointerEvent) {
  if (!isMousePointer(event)) return;
  openPanel();
}

function handleTriggerPointerLeave(event: PointerEvent) {
  if (!isMousePointer(event)) return;
  scheduleClosePanel();
}

function handleContentPointerEnter(event: PointerEvent) {
  if (!isMousePointer(event)) return;
  openPanel();
}

function handleContentPointerLeave(event: PointerEvent) {
  if (!isMousePointer(event)) return;
  scheduleClosePanel();
}

function handleTriggerFocus() {
  if (suppressNextFocusOpen.value) return;
  openPanel();
}

function handleTriggerBlur() {
  suppressNextFocusOpen.value = false;
  scheduleClosePanel();
}

function handleTriggerClick() {
  if (
    isTouchInteraction.value ||
    (lastTriggerPointerType.value !== null &&
      lastTriggerPointerType.value !== "mouse")
  ) {
    clearCloseTimer();
    open.value = false;
    dialogOpen.value = true;
    suppressNextFocusOpen.value = false;
    lastTriggerPointerType.value = null;
    return;
  }

  openPanel();
}

function handleDialogOpenChange(nextOpen: boolean) {
  dialogOpen.value = nextOpen;
  if (nextOpen) {
    clearCloseTimer();
    open.value = false;
  }
}

function openActiveIpDialog() {
  clearCloseTimer();
  open.value = false;
  dialogOpen.value = false;
  activeIpDialogOpen.value = true;
}

async function loadStats() {
  const requestId = ++statsRequestId;
  isStatsLoading.value = true;
  statsError.value = "";
  try {
    const result = await DashboardAPI.getStats(activeRange.value.sec, {
      host: props.host,
    });
    if (requestId !== statsRequestId) return;
    stats.value = result;
  } catch (error: any) {
    if (requestId !== statsRequestId) return;
    statsError.value =
      error?.response?.data?.message ||
      error?.message ||
      t("admin.hostTraffic.loadFailed");
  } finally {
    if (requestId === statsRequestId) {
      isStatsLoading.value = false;
    }
  }
}

watch(
  () => [props.sample, props.timestamp] as const,
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
  () => [open.value, dialogOpen.value, rangeKey.value, props.host] as const,
  ([isPopoverOpen, isDialogOpen]) => {
    if (isPopoverOpen || isDialogOpen) void loadStats();
  },
  { immediate: true },
);

onMounted(() => {
  if (typeof window === "undefined") return;

  interactionMediaQuery = window.matchMedia(
    "(hover: none), (pointer: coarse), (max-width: 767px)",
  );
  updateInteractionMode();

  if (typeof interactionMediaQuery.addEventListener === "function") {
    interactionMediaQuery.addEventListener("change", updateInteractionMode);
    return;
  }

  interactionMediaQuery.addListener(updateInteractionMode);
});

onUnmounted(() => {
  clearCloseTimer();

  if (!interactionMediaQuery) return;

  if (typeof interactionMediaQuery.removeEventListener === "function") {
    interactionMediaQuery.removeEventListener("change", updateInteractionMode);
    return;
  }

  interactionMediaQuery.removeListener(updateInteractionMode);
});
</script>
