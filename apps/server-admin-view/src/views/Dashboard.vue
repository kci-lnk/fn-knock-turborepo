<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRouter } from "vue-router";
import { toast } from "@admin-shared/utils/toast";
import {
  DashboardAPI,
  FrpcAPI,
  CloudflaredAPI,
  ConfigAPI,
  DDNSAPI,
  SecurityAPI,
} from "../lib/api";
import type { DashboardStats, TrafficStats, ThreatOverview } from "../types";
import { isCloudflaredTunnelAvailable } from "../lib/reverse-proxy-submode";
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
  ArrowDownLeft,
  ArrowRight,
  ArrowUpRight,
  Ban,
  Clock,
  Globe,
  Network,
  ShieldAlert,
  TriangleAlert,
  Wifi,
} from "lucide-vue-next";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useTargetPolling } from "../composables/useTargetPolling";
import { useConfigStore } from "../store/config";
import VChart from "vue-echarts";
import type { EChartsOption } from "echarts";
import { use } from "echarts/core";
import { CanvasRenderer } from "echarts/renderers";
import { LineChart } from "echarts/charts";
import { GridComponent, TooltipComponent } from "echarts/components";
import { LegendComponent } from "echarts/components";

use([
  CanvasRenderer,
  LineChart,
  GridComponent,
  TooltipComponent,
  LegendComponent,
]);

const ranges = [
  { key: "15m", label: "15分钟", sec: 15 * 60 },
  { key: "1h", label: "1小时", sec: 60 * 60 },
  { key: "6h", label: "6小时", sec: 6 * 60 * 60 },
  { key: "1d", label: "24小时", sec: 24 * 60 * 60 },
  { key: "7d", label: "7天", sec: 7 * 24 * 60 * 60 },
] as const;

const rangeKey = ref<(typeof ranges)[number]["key"]>("1h");
const isAutoRefresh = ref(true);
const { run: runLoadDashboard } = useAsyncAction();
const isInitializing = ref(true);
const errorMessage = ref("");
const stats = ref<DashboardStats | null>(null);
const threatOverview = ref<ThreatOverview | null>(null);
const lastUpdatedAt = ref<Date | null>(null);
const realtimeStats = ref<TrafficStats | null>(null);
const realtimeInBps = ref<number | null>(null);
const realtimeOutBps = ref<number | null>(null);
let refreshTimer: number | null = null;
let lastRealtimeSample: {
  at: number;
  totalIn: number;
  totalOut: number;
} | null = null;

const router = useRouter();
const configStore = useConfigStore();

type TunnelStatus = {
  running: boolean;
  pid: number | null;
  initialized: boolean;
};

const frpStatus = ref<TunnelStatus | null>(null);
const cfStatus = ref<TunnelStatus | null>(null);
const defaultTunnel = ref<"frp" | "cloudflared">("frp");
const isTunnelInitializing = ref(true);
const { isPending: isTunnelPending, run: runLoadTunnelStatus } =
  useAsyncAction();
const isTunnelLoading = computed(
  () => isTunnelInitializing.value || isTunnelPending.value,
);

const ddnsStatus = ref<{
  enabled: boolean;
  provider: string | null;
  updateScope: "dual_stack" | "ipv6_only" | "ipv4_only";
  lastIP: {
    ipv4: string | null;
    ipv6: string | null;
    updated_at: string | null;
  };
  lastCheck: {
    checked_at: string | null;
    outcome: "updated" | "noop" | "skipped" | "error" | null;
    message: string | null;
  };
} | null>(null);
const isDdnsInitializing = ref(true);
const { isPending: isDdnsPending, run: runLoadDdnsStatus } = useAsyncAction();
const isDdnsLoading = computed(
  () => isDdnsInitializing.value || isDdnsPending.value,
);
const showMainSkeleton = useDelayedLoading(isInitializing);
const showDdnsSkeleton = useDelayedLoading(() => isDdnsLoading.value);
const showTunnelSkeleton = useDelayedLoading(() => isTunnelLoading.value);
const ddnsError = ref("");
const showTunnelSection = computed(() => configStore.config?.run_type === 1);
const showCloudflaredTunnel = computed(() =>
  isCloudflaredTunnelAvailable(configStore.config),
);
const ddnsUpdateScopeLabels = {
  dual_stack: "IPv4 & IPv6",
  ipv6_only: "仅更新 IPv6",
  ipv4_only: "仅更新 IPv4",
} as const;

const loadTunnelStatus = async () => {
  if (!showTunnelSection.value) {
    frpStatus.value = null;
    cfStatus.value = null;
    defaultTunnel.value = "frp";
    isTunnelInitializing.value = false;
    return;
  }

  await runLoadTunnelStatus(
    () =>
      Promise.all([
        FrpcAPI.getStatus().catch(() => null),
        CloudflaredAPI.getStatus().catch(() => null),
        ConfigAPI.getConfig().catch(() => null),
      ]),
    {
      onSuccess: ([frp, cf, config]) => {
        if (frp)
          frpStatus.value = {
            running: frp.running,
            pid: frp.pid,
            initialized: frp.initialized,
          };
        if (cf)
          cfStatus.value = {
            running: cf.running,
            pid: cf.pid,
            initialized: cf.initialized,
          };
        if (config) {
          defaultTunnel.value =
            config.default_tunnel === "cloudflared" &&
            !isCloudflaredTunnelAvailable(config)
              ? "frp"
              : config.default_tunnel || "frp";
        }
      },
      onFinally: () => {
        isTunnelInitializing.value = false;
      },
    },
  );
};

const gotoTunnel = (tab: "frp" | "cloudflared") => {
  router.push({ path: "/tunnel", query: { tab } });
};

const gotoDdns = () => {
  router.push({ path: "/ddns" });
};

const activeRange = computed(
  () => ranges.find((r) => r.key === rangeKey.value) ?? ranges[1],
);

const formatBytes = (bytes: number | null | undefined) => {
  const v = Number(bytes ?? 0);
  if (!Number.isFinite(v) || v <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"] as const;
  const exp = Math.max(
    0,
    Math.min(units.length - 1, Math.floor(Math.log(v) / Math.log(1024))),
  );
  const n = v / 1024 ** exp;
  const digits = exp === 0 ? 0 : n >= 100 ? 0 : n >= 10 ? 1 : 2;
  return `${n.toFixed(digits)} ${units[exp]}`;
};

const formatBps = (bps: number | null | undefined) => `${formatBytes(bps)} /s`;

const formatNumber = (value: number | null | undefined, fallback = "-") => {
  if (value === null || value === undefined) return fallback;
  const v = Number(value);
  if (!Number.isFinite(v)) return fallback;
  return new Intl.NumberFormat("zh-CN").format(Math.round(v));
};

const onlineNow = computed(
  () => realtimeStats.value?.active_conns ?? stats.value?.now?.online ?? null,
);

const trafficOption = computed<EChartsOption>(() => {
  const base = (stats.value?.traffic.echarts ?? {}) as any;
  const colors = ["#171717", "#a3a3a3"]; // 黑白灰色调

  const series = (Array.isArray(base?.series) ? base.series : []).map(
    (s: any, idx: number) => ({
      ...s,
      type: "line",
      smooth: true,
      symbol: "none",
      lineStyle: { width: 2 },
      areaStyle: { opacity: 0.05, color: colors[idx % colors.length] }, // 极淡的底色
      emphasis: { focus: "series" },
    }),
  );

  return {
    ...base,
    color: colors,
    animationDuration: 420,
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "cross" },
      backgroundColor: "rgba(0, 0, 0, 0.85)",
      borderColor: "rgba(255,255,255,0.1)",
      textStyle: { color: "#fff" },
      formatter: (params: any) => {
        const arr = Array.isArray(params) ? params : [params];
        const t = arr[0]?.axisValueLabel ?? "";
        const lines = [t];
        for (const p of arr) {
          const name = String(p?.seriesName ?? "");
          const value = Array.isArray(p?.data) ? p.data[1] : p?.value;
          lines.push(`${p?.marker ?? ""} ${name}: ${formatBps(Number(value))}`);
        }
        return lines.join("<br/>");
      },
    },
    grid: { left: 10, right: 10, top: 24, bottom: 12, containLabel: true },
    xAxis: {
      ...(base?.xAxis ?? {}),
      type: "time",
      boundaryGap: false,
      axisLabel: { color: "#737373" },
      axisLine: { lineStyle: { color: "#e5e5e5" } },
      splitLine: { show: false },
    },
    yAxis: {
      ...(base?.yAxis ?? {}),
      type: "value",
      axisLabel: {
        color: "#737373",
        formatter: (v: number) => formatBytes(v),
      },
      axisLine: { show: false },
      splitLine: { lineStyle: { color: "#f5f5f5" } },
    },
    series,
  } satisfies EChartsOption;
});

const threatOption = computed<EChartsOption>(() => {
  const failedSeries = threatOverview.value?.series.failedLogins ?? [];
  const blockedSeries = threatOverview.value?.series.blockedScanners ?? [];
  const colors = ["#525252", "#171717"];

  return {
    color: colors,
    animationDuration: 420,
    tooltip: {
      trigger: "axis",
      axisPointer: { type: "cross" },
      backgroundColor: "rgba(0, 0, 0, 0.85)",
      borderColor: "rgba(255,255,255,0.1)",
      textStyle: { color: "#fff" },
    },
    grid: { left: 10, right: 10, top: 20, bottom: 12, containLabel: true },
    xAxis: {
      type: "time",
      boundaryGap: false,
      axisLabel: { color: "#737373" },
      axisLine: { lineStyle: { color: "#e5e5e5" } },
      splitLine: { show: false },
    } as any,
    yAxis: {
      type: "value",
      minInterval: 1,
      axisLabel: { color: "#737373" },
      axisLine: { show: false },
      splitLine: { lineStyle: { color: "#f5f5f5" } },
    },
    series: [
      {
        name: "异常登录",
        type: "line",
        smooth: true,
        symbol: "none",
        lineStyle: { width: 2 },
        areaStyle: { opacity: 0.05, color: colors[0] },
        data: failedSeries,
      },
      {
        name: "拦截扫描器",
        type: "line",
        smooth: true,
        symbol: "none",
        lineStyle: { width: 2 },
        areaStyle: { opacity: 0.05, color: colors[1] },
        data: blockedSeries,
      },
    ],
  } satisfies EChartsOption;
});

const applyRealtimeStats = (payload: TrafficStats) => {
  if (
    !payload ||
    !Number.isFinite(payload.total_in) ||
    !Number.isFinite(payload.total_out)
  )
    return;
  realtimeStats.value = payload;
  const now = Number(payload.timestamp ?? Date.now());
  if (lastRealtimeSample) {
    const dt = Math.max(1, (now - lastRealtimeSample.at) / 1000);
    const deltaIn = Math.max(0, payload.total_in - lastRealtimeSample.totalIn);
    const deltaOut = Math.max(
      0,
      payload.total_out - lastRealtimeSample.totalOut,
    );
    realtimeInBps.value = deltaIn / dt;
    realtimeOutBps.value = deltaOut / dt;
  } else {
    realtimeInBps.value = null;
    realtimeOutBps.value = null;
  }
  lastRealtimeSample = {
    at: now,
    totalIn: payload.total_in,
    totalOut: payload.total_out,
  };
};

const realtimePolling = useTargetPolling({
  target: "dashboard",
  intervalMs: 1000,
  onData: (payload) => {
    applyRealtimeStats(payload);
  },
});

const loadDdnsStatus = async () => {
  ddnsError.value = "";
  await runLoadDdnsStatus(() => DDNSAPI.getStatus(), {
    onSuccess: (status) => {
      ddnsStatus.value = status;
    },
    onError: (err: any) => {
      const msg = err?.response?.data?.message || err?.message || "加载失败";
      ddnsError.value = msg;
      ddnsStatus.value = null;
    },
    onFinally: () => {
      isDdnsInitializing.value = false;
    },
  });
};

const load = async () => {
  await runLoadDashboard(
    async () => {
      errorMessage.value = "";
      const [statsRes, threatRes] = await Promise.allSettled([
        DashboardAPI.getStats(activeRange.value.sec),
        SecurityAPI.getOverview(activeRange.value.sec),
      ]);

      if (statsRes.status === "fulfilled") {
        stats.value = statsRes.value;
        lastUpdatedAt.value = new Date();
      } else {
        const msg =
          (statsRes.reason as any)?.response?.data?.message ||
          (statsRes.reason as any)?.message ||
          "加载失败";
        errorMessage.value = msg;
        toast.error("Dashboard 加载失败", { description: msg });
      }

      if (threatRes.status === "fulfilled") {
        threatOverview.value = threatRes.value;
      }
    },
    {
      onError: (err: any) => {
        const msg = err?.response?.data?.message || err?.message || "加载失败";
        errorMessage.value = msg;
        toast.error("Dashboard 加载失败", { description: msg });
      },
      onFinally: () => {
        isInitializing.value = false;
      },
    },
  );
  if (showTunnelSection.value) {
    void loadTunnelStatus();
  } else {
    isTunnelInitializing.value = false;
  }
  void loadDdnsStatus();
};

const refreshAll = () => {
  void load();
};

const startAutoRefresh = () => {
  if (refreshTimer) window.clearInterval(refreshTimer);
  refreshTimer = window.setInterval(() => {
    if (!isAutoRefresh.value) return;
    refreshAll();
  }, 15000);
};

watch(rangeKey, () => {
  void load();
});

watch(showTunnelSection, (visible) => {
  if (visible) {
    isTunnelInitializing.value = true;
    void loadTunnelStatus();
    return;
  }

  frpStatus.value = null;
  cfStatus.value = null;
  defaultTunnel.value = "frp";
  isTunnelInitializing.value = false;
});

watch(isAutoRefresh, () => {
  if (isAutoRefresh.value) startAutoRefresh();
  else if (refreshTimer) window.clearInterval(refreshTimer);
});

onMounted(() => {
  refreshAll();
  realtimePolling.start();
  if (isAutoRefresh.value) startAutoRefresh();
});

onUnmounted(() => {
  if (refreshTimer) window.clearInterval(refreshTimer);
  realtimePolling.stop();
});

const titleRangeText = computed(() => {
  const sec = stats.value?.rangeSec ?? activeRange.value.sec;
  if (sec < 3600) return `${Math.round(sec / 60)} 分钟`;
  if (sec < 24 * 3600) return `${Math.round(sec / 3600)} 小时`;
  return `${Math.round(sec / 86400)} 天`;
});

const metricIconTones = {
  liveIngress: {
    color: "#0f766e",
  },
  liveEgress: {
    color: "#1d4ed8",
  },
  totalIngress: {
    color: "#0369a1",
  },
  totalEgress: {
    color: "#6d28d9",
  },
} as const;

const liveMetricCards = computed(() => [
  {
    label: "实时入站",
    value: realtimeInBps.value === null ? "-" : formatBps(realtimeInBps.value),
    hint: "当前接收速率",
    icon: ArrowDownLeft,
    iconTone: metricIconTones.liveIngress,
  },
  {
    label: "实时出站",
    value:
      realtimeOutBps.value === null ? "-" : formatBps(realtimeOutBps.value),
    hint: "当前发送速率",
    icon: ArrowUpRight,
    iconTone: metricIconTones.liveEgress,
  },
  {
    label: "累计入站",
    value: formatBytes(stats.value?.totals?.inBytes),
    hint: `${titleRangeText.value} 内接收总量`,
    icon: ArrowDownLeft,
    iconTone: metricIconTones.totalIngress,
  },
  {
    label: "累计出站",
    value: formatBytes(stats.value?.totals?.outBytes),
    hint: `${titleRangeText.value} 内发送总量`,
    icon: ArrowUpRight,
    iconTone: metricIconTones.totalEgress,
  },
]);

const securityCards = computed(() => [
  {
    label: "异常登录",
    value: formatNumber(threatOverview.value?.totals?.failedLogins),
    hint: "需复核的登录请求",
    icon: ShieldAlert,
  },
  {
    label: "拦截扫描器",
    value: formatNumber(threatOverview.value?.totals?.blockedScanners),
    hint: "已被加入黑名单",
    icon: Ban,
  },
]);

const ddnsState = computed(() => {
  if (ddnsStatus.value?.enabled) {
    return {
      active: true,
      label: "DDNS 活跃同步",
    };
  }
  return {
    active: false,
    label: "DDNS 已暂停",
  };
});

const ddnsCards = computed(() => [
  {
    label: "提供商",
    value: ddnsStatus.value?.provider || "未配置",
    hint: "动态解析服务",
    icon: Network,
  },
  {
    label: "IPv4",
    value: ddnsStatus.value?.lastIP?.ipv4 || "---.---.---.---",
    hint: "最近上报地址",
    icon: Wifi,
  },
  {
    label: "IPv6",
    value: ddnsStatus.value?.lastIP?.ipv6 || "未检测到地址",
    hint: "最近上报地址",
    icon: Globe,
  },
  {
    label: "更新范围",
    value: ddnsStatus.value
      ? ddnsUpdateScopeLabels[ddnsStatus.value.updateScope]
      : "IPv4 & IPv6",
    hint: "当前生效策略",
    icon: Network,
  },
  {
    label: "最近检查",
    value: ddnsStatus.value?.lastCheck?.checked_at ?? null,
    hint: "自动检查时间",
    icon: Clock,
    isTime: true,
  },
]);

const entryStatusCardTitle = computed(() =>
  showTunnelSection.value ? "入口与隧道" : "入口状态",
);

const entryStatusCardDescription = computed(() =>
  showTunnelSection.value ? "DDNS与穿透状态" : "DDNS 状态",
);

const tunnelCards = computed(() => [
  {
    key: "frp" as const,
    label: "FRP 穿透",
    status: frpStatus.value,
    isDefault: defaultTunnel.value === "frp",
  },
  ...(showCloudflaredTunnel.value
    ? [
        {
          key: "cloudflared" as const,
          label: "Cloudflared",
          status: cfStatus.value,
          isDefault: defaultTunnel.value === "cloudflared",
        },
      ]
    : []),
]);
</script>

<template>
  <div class="h-full flex flex-col gap-6">
    <section
      class="flex flex-col xl:flex-row xl:items-baseline xl:justify-between gap-6"
      data-guide-onboarding-done
    >
      <div class="space-y-2 min-w-0">
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground"
        >
          <span>范围: {{ titleRangeText }}</span>
          <span class="text-border">|</span>
          <span class="font-medium text-foreground"
            >在线: {{ formatNumber(onlineNow ? onlineNow : 0) }}</span
          >
        </div>
      </div>

      <div
        class="flex flex-col sm:flex-row items-stretch sm:items-center gap-3"
      >
        <Tabs v-model="rangeKey" class="w-full sm:w-auto">
          <TabsList class="grid w-full grid-cols-5 sm:w-auto">
            <TabsTrigger
              v-for="r in ranges"
              :key="r.key"
              :value="r.key"
              class="px-3 text-xs sm:text-sm"
            >
              {{ r.label }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
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
      <AlertTitle>加载失败</AlertTitle>
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
                <CardTitle class="text-lg">安全拦截</CardTitle>
                <CardDescription class="mt-1">扫描与异常活动</CardDescription>
              </div>
            </div>
          </CardHeader>
          <CardContent class="pt-0 space-y-4">
            <div v-if="isInitializing && showMainSkeleton" class="space-y-3">
              <div class="grid gap-3 sm:grid-cols-2">
                <Skeleton class="h-[84px] w-full rounded-xl" />
                <Skeleton class="h-[84px] w-full rounded-xl" />
              </div>
              <Skeleton class="h-[180px] w-full rounded-xl" />
            </div>
            <div v-else-if="!isInitializing" class="space-y-4">
              <div class="grid gap-3 sm:grid-cols-2">
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
                <VChart
                  :option="threatOption"
                  autoresize
                  class="h-full w-full"
                />
              </div>
            </div>
            <div v-else class="h-[310px]" aria-hidden="true"></div>
          </CardContent>
        </Card>

        <Card class="border bg-card shadow-none rounded-xl">
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
                  <div class="text-sm font-medium">DDNS 状态</div>
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
                  >管理</Button
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
                      empty-text="从未"
                      :keep-invalid-raw-text="false"
                    />
                    <template v-else>{{ item.value }}</template>
                  </div>
                </div>
              </div>
              <div v-else class="h-[68px]" aria-hidden="true"></div>
            </div>

            <div v-if="showTunnelSection">
              <div class="mb-3 text-sm font-medium">隧道入口</div>
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
                        默认
                      </Badge>
                    </div>
                    <div
                      class="mt-1.5 flex items-center gap-2 text-xs text-muted-foreground"
                    >
                      <LiveStatusBadge
                        :active="Boolean(item.status?.running)"
                        active-label="运行中"
                        inactive-label="未运行"
                        size="xs"
                      />
                      <span>{{
                        item.status?.running ? "运行中" : "未运行"
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
              <CardTitle class="text-lg">网络流量走势</CardTitle>
              <CardDescription class="mt-1">时间序列的吞吐变化</CardDescription>
            </div>
          </div>
        </CardHeader>
        <CardContent>
          <div v-if="isInitializing && showMainSkeleton">
            <Skeleton class="h-[300px] w-full rounded-xl" />
          </div>
          <div v-else-if="!isInitializing" class="h-[300px] w-full">
            <VChart :option="trafficOption" autoresize class="h-full w-full" />
          </div>
          <div v-else class="h-[300px]" aria-hidden="true"></div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>
