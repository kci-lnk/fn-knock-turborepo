<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "@admin-shared/utils/toast";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Button } from "@/components/ui/button";
import LiveStatusBadge from "@/components/LiveStatusBadge.vue";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Skeleton } from "@/components/ui/skeleton";
import { Badge } from "@/components/ui/badge";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
  DialogTrigger,
} from "@/components/ui/dialog";
import { ArrowRight, Check, Palette, TriangleAlert } from "lucide-vue-next";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useConfigStore } from "../store/config";
import { useDashboardTunnelStatus } from "./dashboard/useDashboardTunnelStatus";
import { useDashboardRealtimeTraffic } from "./dashboard/useDashboardRealtimeTraffic";
import { useDashboardViewModel } from "./dashboard/useDashboardViewModel";
import {
  dashboardRanges,
  useDashboardData,
} from "./dashboard/useDashboardData";
import TimeSeriesChart from "@/components/charts/TimeSeriesChart.vue";
import {
  THEME_COLOR_PRESETS,
  normalizeAppearanceConfig,
  type ThemeColorPresetKey,
} from "@frontend-core/appearance";

const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();
const ranges = dashboardRanges;
const themeDialogOpen = ref(false);
const isSavingThemePreset = ref(false);

const showTunnelSection = computed(
  () =>
    configStore.config?.run_type === 1 &&
    (configStore.canUseFrpc || configStore.canUseCloudflared),
);
const {
  cfStatus,
  defaultTunnel,
  dispose: disposeTunnelStatus,
  frpStatus,
  isLoading: isTunnelLoading,
  reset: resetTunnelStatus,
  scheduleLoad: scheduleTunnelStatusLoad,
} = useDashboardTunnelStatus({
  getConfig: () => configStore.config,
  loadConfig: () => configStore.loadConfig(),
  showTunnelSection,
});
const showTunnelSkeleton = useDelayedLoading(() => isTunnelLoading.value);
const showEntryStatusModule = computed(
  () =>
    configStore.config?.dashboard_display?.show_entry_status_module !== false,
);
const gotoTunnel = (tab: "frp" | "cloudflared") => {
  void router.push({ path: "/tunnel", query: { tab } });
};
const gotoDdns = () => {
  void router.push({ path: "/ddns" });
};

const {
  polling: realtimePolling,
  realtimeInBps,
  realtimeOutBps,
  realtimeStats,
} = useDashboardRealtimeTraffic();

const {
  activeRange,
  ddnsStatus,
  errorMessage,
  isDdnsLoading,
  isInitializing,
  rangeKey,
  showDdnsSkeleton,
  showMainSkeleton,
  stats,
  threatOverview,
} = useDashboardData({
  disposeTunnelStatus,
  scheduleTunnelStatusLoad,
  startRealtimePolling: realtimePolling.start,
  stopRealtimePolling: realtimePolling.stop,
  translate: (key) => t(key),
});

const themePresetOptions = THEME_COLOR_PRESETS.map((preset) => ({
  ...preset,
  labelKey: `admin.dashboard.theme.presets.${preset.key}`,
}));
const activeThemeColorPreset = computed(
  () =>
    normalizeAppearanceConfig(configStore.config?.appearance)
      .theme_color_preset,
);
const getErrorMessage = (error: unknown, fallback: string) => {
  const value = error as {
    response?: { data?: { message?: string } };
    message?: string;
  };
  return value?.response?.data?.message || value?.message || fallback;
};
const selectThemeColorPreset = async (preset: ThemeColorPresetKey) => {
  if (preset === activeThemeColorPreset.value || isSavingThemePreset.value) {
    return;
  }
  isSavingThemePreset.value = true;
  try {
    await configStore.saveAppearanceConfig({ theme_color_preset: preset });
    themeDialogOpen.value = false;
  } catch (error) {
    toast.error(t("admin.dashboard.theme.saveFailed"), {
      description: getErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSavingThemePreset.value = false;
  }
};

const {
  ddnsCards,
  ddnsState,
  entryStatusCardDescription,
  entryStatusCardTitle,
  formatBps,
  formatNumber,
  liveMetricCards,
  onlineNow,
  securityCards,
  threatSeries,
  titleRangeText,
  trafficSeries,
  tunnelCards,
} = useDashboardViewModel({
  activeRangeSec: () => activeRange.value.sec,
  cfStatus,
  ddnsStatus,
  defaultTunnel,
  frpStatus,
  realtimeInBps,
  realtimeOutBps,
  realtimeStats,
  showTunnelSection,
  stats,
  threatOverview,
});

watch(showTunnelSection, (visible) => {
  if (visible) {
    scheduleTunnelStatusLoad();
  } else {
    resetTunnelStatus();
  }
});
</script>

<template>
  <div class="h-full flex flex-col gap-6">
    <section
      class="flex flex-col xl:flex-row xl:items-baseline xl:justify-between gap-6"
    >
      <div class="space-y-2 min-w-0">
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground"
        >
          <span
            >{{ t("admin.dashboard.labels.range") }}: {{ titleRangeText }}</span
          >
          <span class="text-border">|</span>
          <span class="font-medium text-foreground"
            >{{ t("admin.dashboard.labels.online") }}:
            {{ formatNumber(onlineNow ? onlineNow : 0) }}</span
          >
        </div>
      </div>

      <div
        class="flex flex-col sm:flex-row items-stretch sm:items-center gap-3"
      >
        <div class="flex w-full items-center gap-2 sm:w-auto">
          <Tabs v-model="rangeKey" class="min-w-0 flex-1 sm:w-auto">
            <TabsList class="grid w-full grid-cols-5 sm:w-auto">
              <TabsTrigger
                v-for="r in ranges"
                :key="r.key"
                :value="r.key"
                class="px-3 text-xs sm:text-sm"
              >
                {{ t(r.labelKey) }}
              </TabsTrigger>
            </TabsList>
          </Tabs>

          <Dialog v-model:open="themeDialogOpen">
            <DialogTrigger as-child>
              <Button
                variant="ghost"
                size="icon"
                class="h-9 w-9 shrink-0 rounded-lg bg-muted p-[3px] text-foreground shadow-none hover:bg-muted hover:text-foreground"
                :aria-label="t('admin.dashboard.theme.buttonLabel')"
                :title="t('admin.dashboard.theme.buttonLabel')"
              >
                <span
                  class="inline-flex h-full w-full items-center justify-center rounded-md bg-background shadow-sm"
                >
                  <Palette class="h-4 w-4" />
                </span>
              </Button>
            </DialogTrigger>
            <DialogContent class="sm:max-w-[460px]">
              <DialogHeader>
                <DialogTitle>{{
                  t("admin.dashboard.theme.title")
                }}</DialogTitle>
                <DialogDescription>
                  {{ t("admin.dashboard.theme.description") }}
                </DialogDescription>
              </DialogHeader>

              <div class="grid gap-2">
                <Button
                  v-for="preset in themePresetOptions"
                  :key="preset.key"
                  type="button"
                  variant="outline"
                  class="h-auto justify-start gap-3 px-3 py-3 text-left"
                  :class="
                    preset.key === activeThemeColorPreset
                      ? 'border-primary bg-primary/5 ring-1 ring-primary/20'
                      : 'border-border/70 hover:border-primary/35'
                  "
                  :disabled="isSavingThemePreset"
                  @click="selectThemeColorPreset(preset.key)"
                >
                  <span
                    class="size-5 shrink-0 rounded-full border border-border shadow-sm"
                    :style="{ backgroundColor: preset.color }"
                  />
                  <span class="min-w-0 flex-1 text-sm font-medium">
                    {{ t(preset.labelKey) }}
                  </span>
                  <Check
                    v-if="preset.key === activeThemeColorPreset"
                    class="h-4 w-4 shrink-0 text-primary"
                  />
                </Button>
              </div>
            </DialogContent>
          </Dialog>
        </div>
      </div>
    </section>

    <section class="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
      <div
        v-for="item in liveMetricCards"
        :key="item.label"
        class="rounded-xl border bg-card p-5 shadow-none"
      >
        <div class="flex items-start justify-between gap-3">
          <div>
            <div class="text-sm font-medium text-muted-foreground">
              {{ item.label }}
            </div>
            <div class="mt-2 text-2xl font-semibold tracking-tight">
              {{ item.value }}
            </div>
          </div>
          <component
            :is="item.icon"
            class="h-4 w-4"
            :style="{ color: item.iconTone.color }"
          />
        </div>
        <div class="mt-3 text-xs text-muted-foreground">{{ item.hint }}</div>
      </div>
    </section>

    <Alert v-if="errorMessage" variant="destructive" class="rounded-xl">
      <TriangleAlert class="h-4 w-4" />
      <AlertTitle>{{ t("admin.dashboard.errors.loadFailed") }}</AlertTitle>
      <AlertDescription>{{ errorMessage }}</AlertDescription>
    </Alert>

    <div class="space-y-4">
      <div
        class="grid gap-4 [grid-template-columns:repeat(auto-fit,minmax(min(100%,24rem),1fr))]"
      >
        <Card class="border bg-card shadow-none rounded-xl">
          <CardHeader class="pb-3">
            <div class="flex items-start justify-between gap-3">
              <div>
                <CardTitle class="text-lg">{{
                  t("admin.dashboard.security.title")
                }}</CardTitle>
                <CardDescription class="mt-1">{{
                  t("admin.dashboard.security.description")
                }}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent class="pt-0 space-y-4">
            <div v-if="isInitializing && showMainSkeleton" class="space-y-3">
              <div class="grid gap-3 sm:grid-cols-3">
                <Skeleton class="h-[84px] w-full rounded-xl" />
                <Skeleton class="h-[84px] w-full rounded-xl" />
                <Skeleton class="h-[84px] w-full rounded-xl" />
              </div>
              <Skeleton class="h-[180px] w-full rounded-xl" />
            </div>
            <div v-else-if="!isInitializing" class="space-y-4">
              <div class="grid gap-3 sm:grid-cols-3">
                <div
                  v-for="item in securityCards"
                  :key="item.label"
                  class="rounded-xl border bg-muted/20 px-4 py-3"
                >
                  <div
                    class="flex items-center justify-between gap-3 text-sm font-medium text-muted-foreground"
                  >
                    {{ item.label }}
                    <component :is="item.icon" class="h-4 w-4" />
                  </div>
                  <div class="mt-2 text-xl font-semibold">
                    {{ item.value }}
                  </div>
                </div>
              </div>
              <div class="h-[180px] w-full">
                <TimeSeriesChart
                  :series="threatSeries"
                  :value-formatter="(value) => formatNumber(value, '0')"
                  class="h-full w-full"
                />
              </div>
            </div>
            <div v-else class="h-[310px]" aria-hidden="true"></div>
          </CardContent>
        </Card>

        <Card
          v-if="showEntryStatusModule"
          class="border bg-card shadow-none rounded-xl"
        >
          <CardHeader class="pb-3">
            <div class="flex items-start justify-between">
              <div>
                <CardTitle class="text-lg">{{
                  entryStatusCardTitle
                }}</CardTitle>
                <CardDescription class="mt-1">{{
                  entryStatusCardDescription
                }}</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent class="space-y-5">
            <div>
              <div class="mb-3 flex items-center justify-between">
                <div class="flex items-center gap-2">
                  <div class="text-sm font-medium">
                    {{ t("admin.dashboard.ddns.statusTitle") }}
                  </div>
                  <LiveStatusBadge
                    :active="ddnsState.active"
                    :active-label="ddnsState.label"
                    :inactive-label="ddnsState.label"
                    class="mt-px"
                  />
                </div>
                <Button
                  variant="ghost"
                  size="sm"
                  class="h-7 text-xs"
                  @click="gotoDdns"
                  >{{ t("admin.dashboard.labels.manage") }}</Button
                >
              </div>
              <div
                v-if="isDdnsLoading && showDdnsSkeleton"
                class="grid gap-3 sm:grid-cols-2"
              >
                <Skeleton class="h-[68px] w-full rounded-xl" />
                <Skeleton class="h-[68px] w-full rounded-xl" />
              </div>
              <div v-else-if="!isDdnsLoading" class="grid gap-3 sm:grid-cols-2">
                <div
                  v-for="item in ddnsCards"
                  :key="item.label"
                  class="rounded-xl border bg-muted/20 px-3 py-2.5"
                >
                  <div
                    class="flex items-center gap-2 text-xs text-muted-foreground"
                  >
                    <component :is="item.icon" class="h-3.5 w-3.5" />
                    {{ item.label }}
                  </div>
                  <div
                    class="mt-1 truncate text-sm font-medium"
                    :title="item.value ?? undefined"
                  >
                    <HumanFriendlyTime
                      v-if="item.isTime"
                      :value="item.value"
                      :empty-text="t('admin.ddns.never')"
                      :keep-invalid-raw-text="false"
                      :tooltip-lines="item.tooltipLines"
                    />
                    <template v-else>{{ item.value }}</template>
                  </div>
                </div>
              </div>
              <div v-else class="h-[68px]" aria-hidden="true"></div>
            </div>

            <div v-if="showTunnelSection">
              <div class="mb-3 text-sm font-medium">
                {{ t("admin.dashboard.tunnel.title") }}
              </div>
              <div
                v-if="isTunnelLoading && showTunnelSkeleton"
                class="grid gap-3"
              >
                <Skeleton class="h-[60px] w-full rounded-xl" />
                <Skeleton class="h-[60px] w-full rounded-xl" />
              </div>
              <div v-else-if="!isTunnelLoading" class="grid gap-3">
                <button
                  v-for="item in tunnelCards"
                  :key="item.key"
                  type="button"
                  class="group flex items-center justify-between rounded-xl border bg-muted/20 px-4 py-3 text-left transition-colors hover:bg-muted/50"
                  @click="gotoTunnel(item.key)"
                >
                  <div>
                    <div class="flex items-center gap-2">
                      <div class="text-sm font-medium">{{ item.label }}</div>
                      <Badge
                        v-if="item.isDefault"
                        variant="outline"
                        class="rounded-sm px-1.5 py-0 text-[10px]"
                      >
                        {{ t("admin.dashboard.tunnel.default") }}
                      </Badge>
                    </div>
                    <div
                      class="mt-1.5 flex items-center gap-2 text-xs text-muted-foreground"
                    >
                      <LiveStatusBadge
                        :active="Boolean(item.status?.running)"
                        :active-label="t('admin.dashboard.tunnel.running')"
                        :inactive-label="t('admin.dashboard.tunnel.notRunning')"
                        size="xs"
                      />
                      <span>{{
                        item.status?.running
                          ? t("admin.dashboard.tunnel.running")
                          : t("admin.dashboard.tunnel.notRunning")
                      }}</span>
                      <span v-if="item.status?.running && item.status.pid"
                        >PID {{ item.status.pid }}</span
                      >
                    </div>
                  </div>
                  <ArrowRight
                    class="h-4 w-4 text-muted-foreground transition-transform group-hover:translate-x-0.5"
                  />
                </button>
              </div>
              <div v-else class="h-[60px]" aria-hidden="true"></div>
            </div>
          </CardContent>
        </Card>
      </div>

      <Card class="border bg-card shadow-none rounded-xl">
        <CardHeader class="pb-3">
          <div class="flex items-center justify-between">
            <div>
              <CardTitle class="text-lg">{{
                t("admin.dashboard.traffic.title")
              }}</CardTitle>
              <CardDescription class="mt-1">{{
                t("admin.dashboard.traffic.description")
              }}</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div v-if="isInitializing && showMainSkeleton">
            <Skeleton class="h-[300px] w-full rounded-xl" />
          </div>
          <div v-else-if="!isInitializing" class="h-[300px] w-full">
            <TimeSeriesChart
              :series="trafficSeries"
              :value-formatter="formatBps"
              class="h-full w-full"
            />
          </div>
          <div v-else class="h-[300px]" aria-hidden="true"></div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>
