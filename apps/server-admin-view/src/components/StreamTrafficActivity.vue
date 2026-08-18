<template>
  <Popover :open="open" @update:open="handleOpenChange">
    <PopoverAnchor as-child>
      <button
        type="button"
        class="inline-flex min-h-6 max-w-full flex-wrap items-center gap-x-2 gap-y-1 px-1.5 text-left text-xs leading-none transition-colors hover:bg-muted/50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
        :class="{ 'border-primary/30 bg-primary/5': open || dialogOpen }"
        :aria-label="
          t('admin.streamTraffic.detailsAria', {
            title: displayTitle,
            key: streamKey,
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
      <div class="flex items-start justify-between gap-3 border-b px-4 py-3">
        <div class="min-w-0">
          <div class="truncate text-sm font-semibold" :title="displayTitle">
            {{ displayTitle }}
          </div>
          <div
            class="mt-1 break-all font-mono text-xs font-medium text-muted-foreground"
            :title="streamKey"
          >
            {{ streamKey }}
          </div>
          <div class="mt-1.5 flex flex-wrap items-center gap-2 text-xs">
            <span
              class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-muted-foreground"
            >
              <Activity class="h-3 w-3 shrink-0" />
              {{ activeConnsText }}
            </span>
            <span class="text-muted-foreground">{{ sampleStatusText }}</span>
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
      <DialogHeader
        class="flex-row items-start justify-between gap-3 border-b px-4 py-3 pr-10 text-left"
      >
        <div class="min-w-0">
          <DialogTitle class="truncate text-base" :title="displayTitle">
            {{ displayTitle }}
          </DialogTitle>
          <DialogDescription class="space-y-1 text-left">
            <span class="block break-all font-mono">{{ streamKey }}</span>
            <span class="flex items-center gap-2 text-xs">
              <span
                class="inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5"
              >
                <Activity class="h-3 w-3 shrink-0" />
                {{ activeConnsText }}
              </span>
              <span>{{ sampleStatusText }}</span>
            </span>
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
    :host="streamKey"
    :items="activeIpItems"
    :loading="activeIpLoading"
    :error="activeIpError"
    :updated-at="activeIpUpdatedAt"
    :window-seconds="activeIpWindowSeconds"
    @refresh="refreshActiveIps"
  />
</template>

<script setup lang="ts">
import { computed, ref, toRef } from "vue";
import { useI18n } from "vue-i18n";
import {
  Activity,
  ArrowDownLeft,
  ArrowUpRight,
  Network,
} from "lucide-vue-next";
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
import TimeSeriesChart from "@/components/charts/TimeSeriesChart.vue";
import { useStreamActiveIps } from "@/composables/useHostActiveIps";
import type { StreamTrafficStats } from "@/types";
import { useHostTrafficOverlayInteraction } from "./host-traffic/useHostTrafficOverlayInteraction";
import { useStreamTrafficStats } from "./stream-traffic/useStreamTrafficStats";

const props = withDefaults(
  defineProps<{
    streamKey: string;
    title?: string | null;
    sample?: StreamTrafficStats | null;
    timestamp?: number | null;
  }>(),
  {
    title: "",
    sample: null,
    timestamp: null,
  },
);

const { t } = useI18n();
const activeIpDialogOpen = ref(false);

const {
  displayItems: activeIpItems,
  loading: activeIpLoading,
  error: activeIpError,
  updatedAt: activeIpUpdatedAt,
  windowSeconds: activeIpWindowSeconds,
  refresh: refreshActiveIps,
} = useStreamActiveIps(
  computed(() => props.streamKey),
  activeIpDialogOpen,
);

const {
  closeOverlays,
  dialogOpen,
  handleContentPointerEnter,
  handleContentPointerLeave,
  handleDialogOpenChange,
  handleOpenChange,
  handleTriggerBlur,
  handleTriggerClick,
  handleTriggerFocus,
  handleTriggerPointerDown,
  handleTriggerPointerEnter,
  handleTriggerPointerLeave,
  isTouchInteraction,
  open,
} = useHostTrafficOverlayInteraction();

const {
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
} = useStreamTrafficStats({
  active: computed(() => open.value || dialogOpen.value),
  stream: toRef(props, "streamKey"),
  sample: toRef(props, "sample"),
  timestamp: toRef(props, "timestamp"),
});

const hasRealtimeSample = computed(() => Boolean(props.sample));
const displayTitle = computed(
  () =>
    props.title?.trim() ||
    props.streamKey ||
    t("admin.hostTraffic.unknownTitle"),
);
const sampleStatusText = computed(() =>
  hasRealtimeSample.value
    ? t("admin.hostTraffic.sampling")
    : t("admin.hostTraffic.waitingSample"),
);
const activeConns = computed(() => Number(props.sample?.active_conns ?? 0));
const activeConnsText = computed(() =>
  activeConns.value > 0
    ? t("admin.streamTraffic.activeConnsWithCount", {
        count: activeConns.value,
      })
    : t("admin.streamTraffic.activeConns"),
);

const activeIpButtonText = computed(() => {
  const count = Number(
    props.sample?.active_ip_count ?? activeIpItems.value.length,
  );
  return count > 0
    ? t("admin.hostTraffic.activeIpWithCount", { count })
    : t("admin.hostTraffic.activeIp");
});

function openActiveIpDialog() {
  closeOverlays();
  activeIpDialogOpen.value = true;
}
</script>
