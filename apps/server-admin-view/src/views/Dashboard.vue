<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { toast } from "@admin-shared/utils/toast";
import {
  DashboardAPI,
  FrpcAPI,
  CloudflaredAPI,
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
  Route as RouteIcon,
  ShieldAlert,
  TriangleAlert,
  Wifi,
} from "lucide-vue-next";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import { useTargetPolling } from "../composables/useTargetPolling";
import { useConfigStore } from "../store/config";
import { buildDDNSTimestampTooltipLines } from "../lib/ddns-time";
import TimeSeriesChart, {
  type TimeSeriesChartSeries,
} from "@/components/charts/TimeSeriesChart.vue";

const ranges = [
  {
    key: "15m",
    labelKey: "admin.dashboard.ranges.fifteenMinutes",
    sec: 15 * 60,
  },
  { key: "1h", labelKey: "admin.dashboard.ranges.oneHour", sec: 60 * 60 },
  {
    key: "6h",
    labelKey: "admin.dashboard.ranges.sixHours",
    sec: 6 * 60 * 60,
  },
  {
    key: "1d",
    labelKey: "admin.dashboard.ranges.twentyFourHours",
    sec: 24 * 60 * 60,
  },
  {
    key: "7d",
    labelKey: "admin.dashboard.ranges.sevenDays",
    sec: 7 * 24 * 60 * 60,
  },
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
let tunnelStatusTimer: number | null = null;
let tunnelStatusInFlight: Promise<void> | null = null;

const router = useRouter();
const configStore = useConfigStore();
const { locale, t } = useI18n();

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
  extraTargetCount: number;
  enabledExtraTargetCount: number;
  targets: Array<{
    id: string;
    isPrimary: boolean;
    lastCheck: {
      outcome: "updated" | "noop" | "skipped" | "error" | null;
    };
  }>;
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
const showEntryStatusModule = computed(
  () =>
    configStore.config?.dashboard_display?.show_entry_status_module !== false,
);
const showCloudflaredTunnel = computed(() =>
  isCloudflaredTunnelAvailable(configStore.config),
);
const ddnsUpdateScopeLabelKeys = {
  dual_stack: "admin.dashboard.ddns.updateScopes.dualStack",
  ipv6_only: "admin.dashboard.ddns.updateScopes.ipv6Only",
  ipv4_only: "admin.dashboard.ddns.updateScopes.ipv4Only",
} as const;

const trafficSeriesLabelKeys: Record<string, string> = {
  "\u5165\u7AD9": "admin.dashboard.traffic.ingressSeries",
  "\u51FA\u7AD9": "admin.dashboard.traffic.egressSeries",
};

const getDdnsTimestampLabels = () => ({
  lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
  lastCheck: t("admin.ddns.lastCheck"),
  never: t("admin.ddns.never"),
});

const translateTrafficSeriesName = (name: unknown) => {
  const value = String(name ?? "");
  const key = trafficSeriesLabelKeys[value];
  return key ? t(key) : value;
};

const resetTunnelStatus = () => {
  frpStatus.value = null;
  cfStatus.value = null;
  defaultTunnel.value = "frp";
  isTunnelInitializing.value = false;
};

const runTunnelStatusLoad = async () => {
  if (!showTunnelSection.value) {
    resetTunnelStatus();
    return;
  }

  await runLoadTunnelStatus(
    () =>
      Promise.all([
        FrpcAPI.getStatus().catch(() => null),
        CloudflaredAPI.getStatus().catch(() => null),
        (configStore.config
          ? Promise.resolve(configStore.config)
          : configStore.loadConfig()
        ).catch(() => null),
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

const loadTunnelStatus = async () => {
  if (tunnelStatusInFlight) return tunnelStatusInFlight;

  tunnelStatusInFlight = runTunnelStatusLoad().finally(() => {
    tunnelStatusInFlight = null;
  });
  return tunnelStatusInFlight;
};

const scheduleTunnelStatusLoad = () => {
  if (tunnelStatusTimer !== null) {
    window.clearTimeout(tunnelStatusTimer);
    tunnelStatusTimer = null;
  }

  if (!showTunnelSection.value) {
    resetTunnelStatus();
    return;
  }

  if (tunnelStatusInFlight) return;
  isTunnelInitializing.value = true;
  tunnelStatusTimer = window.setTimeout(() => {
    tunnelStatusTimer = null;
    void loadTunnelStatus();
  }, 0);
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
  return new Intl.NumberFormat(String(locale.value)).format(Math.round(v));
};

const onlineNow = computed(
  () => realtimeStats.value?.active_conns ?? stats.value?.now?.online ?? null,
);

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
  const colors = ["#0f766e", "#2563eb"];

  return (Array.isArray(base?.series) ? base.series : []).map(
    (s: any, idx: number) => {
      const color = colors[idx % colors.length] ?? "#0f766e";
      return {
        name: translateTrafficSeriesName(s?.name),
        color,
        fill: `${color}14`,
        data: normalizeSeriesData(s?.data),
      };
    },
  );
});

const threatSeries = computed<TimeSeriesChartSeries[]>(() => {
  const failedSeries = threatOverview.value?.series.failedLogins ?? [];
  const blockedSeries = threatOverview.value?.series.blockedScanners ?? [];
  const wafSeries = threatOverview.value?.series.wafEvents ?? [];
  return [
    {
      name: t("admin.dashboard.security.failedLogins"),
      color: "#525252",
      fill: "rgba(82, 82, 82, 0.08)",
      data: failedSeries,
    },
    {
      name: t("admin.dashboard.security.scanners"),
      color: "#991b1b",
      fill: "rgba(153, 27, 27, 0.08)",
      data: blockedSeries,
    },
    {
      name: "WAF",
      color: "#b45309",
      fill: "rgba(180, 83, 9, 0.08)",
      data: wafSeries,
    },
  ];
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
      const msg =
        err?.response?.data?.message ||
        err?.message ||
        t("admin.dashboard.errors.loadFailed");
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
          t("admin.dashboard.errors.loadFailed");
        errorMessage.value = msg;
        toast.error(t("admin.dashboard.errors.dashboardLoadFailed"), {
          description: msg,
        });
      }

      if (threatRes.status === "fulfilled") {
        threatOverview.value = threatRes.value;
      }
    },
    {
      onError: (err: any) => {
        const msg =
          err?.response?.data?.message ||
          err?.message ||
          t("admin.dashboard.errors.loadFailed");
        errorMessage.value = msg;
        toast.error(t("admin.dashboard.errors.dashboardLoadFailed"), {
          description: msg,
        });
      },
      onFinally: () => {
        isInitializing.value = false;
      },
    },
  );
  scheduleTunnelStatusLoad();
  window.setTimeout(() => {
    void loadDdnsStatus();
  }, 0);
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
    scheduleTunnelStatusLoad();
    return;
  }

  resetTunnelStatus();
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
  if (tunnelStatusTimer !== null) window.clearTimeout(tunnelStatusTimer);
  realtimePolling.stop();
});

const titleRangeText = computed(() => {
  const sec = stats.value?.rangeSec ?? activeRange.value.sec;
  if (sec < 3600) {
    return t("admin.dashboard.duration.minutes", {
      count: Math.round(sec / 60),
    });
  }
  if (sec < 24 * 3600) {
    return t("admin.dashboard.duration.hours", {
      count: Math.round(sec / 3600),
    });
  }
  return t("admin.dashboard.duration.days", {
    count: Math.round(sec / 86400),
  });
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
    label: t("admin.dashboard.metrics.liveIngress"),
    value: realtimeInBps.value === null ? "-" : formatBps(realtimeInBps.value),
    hint: t("admin.dashboard.metrics.currentReceiveRate"),
    icon: ArrowDownLeft,
    iconTone: metricIconTones.liveIngress,
  },
  {
    label: t("admin.dashboard.metrics.liveEgress"),
    value:
      realtimeOutBps.value === null ? "-" : formatBps(realtimeOutBps.value),
    hint: t("admin.dashboard.metrics.currentSendRate"),
    icon: ArrowUpRight,
    iconTone: metricIconTones.liveEgress,
  },
  {
    label: t("admin.dashboard.metrics.totalIngress"),
    value: formatBytes(stats.value?.totals?.inBytes),
    hint: t("admin.dashboard.metrics.rangeReceiveTotal", {
      range: titleRangeText.value,
    }),
    icon: ArrowDownLeft,
    iconTone: metricIconTones.totalIngress,
  },
  {
    label: t("admin.dashboard.metrics.totalEgress"),
    value: formatBytes(stats.value?.totals?.outBytes),
    hint: t("admin.dashboard.metrics.rangeSendTotal", {
      range: titleRangeText.value,
    }),
    icon: ArrowUpRight,
    iconTone: metricIconTones.totalEgress,
  },
]);

const securityCards = computed(() => [
  {
    label: t("admin.dashboard.security.failedLogins"),
    value: formatNumber(threatOverview.value?.totals?.failedLogins),
    hint: t("admin.dashboard.security.failedLoginsHint"),
    icon: ShieldAlert,
  },
  {
    label: t("admin.dashboard.security.scanners"),
    value: formatNumber(threatOverview.value?.totals?.blockedScanners),
    hint: t("admin.dashboard.security.scannersHint"),
    icon: Ban,
  },
  {
    label: "WAF",
    value: formatNumber(threatOverview.value?.totals?.wafEvents),
    hint: t("admin.dashboard.security.wafHint"),
    icon: TriangleAlert,
  },
]);

const ddnsState = computed(() => {
  if (ddnsStatus.value?.enabled) {
    return {
      active: true,
      label: t("admin.dashboard.ddns.activeSync"),
    };
  }
  return {
    active: false,
    label: t("admin.dashboard.ddns.paused"),
  };
});

const ddnsCards = computed(() => [
  {
    label: t("admin.dashboard.ddns.provider"),
    value: ddnsStatus.value?.provider || t("admin.dashboard.ddns.notConfigured"),
    hint:
      (ddnsStatus.value?.extraTargetCount || 0) > 0
        ? t("admin.dashboard.ddns.primaryDynamicServiceWithExtra", {
            count: ddnsStatus.value?.extraTargetCount || 0,
          })
        : t("admin.dashboard.ddns.primaryDynamicService"),
    icon: Network,
  },
  {
    label: "IPv4",
    value: ddnsStatus.value?.lastIP?.ipv4 || "---.---.---.---",
    hint: t("admin.dashboard.ddns.lastReportedAddress"),
    icon: Wifi,
  },
  {
    label: "IPv6",
    value:
      ddnsStatus.value?.lastIP?.ipv6 ||
      t("admin.dashboard.ddns.noAddressDetected"),
    hint: t("admin.dashboard.ddns.lastReportedAddress"),
    icon: Globe,
  },
  {
    label: t("admin.dashboard.ddns.updateScope"),
    value: ddnsStatus.value
      ? t(ddnsUpdateScopeLabelKeys[ddnsStatus.value.updateScope])
      : "IPv4 & IPv6",
    hint: t("admin.dashboard.ddns.activePolicy"),
    icon: RouteIcon,
  },
  {
    label: t("admin.dashboard.ddns.lastCheck"),
    value: ddnsStatus.value?.lastCheck?.checked_at ?? null,
    hint: t("admin.dashboard.ddns.autoCheckTime"),
    icon: Clock,
    isTime: true,
    tooltipLines: buildDDNSTimestampTooltipLines({
      updatedAt: ddnsStatus.value?.lastIP?.updated_at,
      checkedAt: ddnsStatus.value?.lastCheck?.checked_at,
      locale: String(locale.value),
      labels: getDdnsTimestampLabels(),
    }),
  },
  {
    label: t("admin.dashboard.ddns.extraDomains"),
    value: String(ddnsStatus.value?.extraTargetCount || 0),
    hint:
      (ddnsStatus.value?.targets || []).filter(
        (target) => !target.isPrimary && target.lastCheck.outcome === "error",
      ).length > 0
        ? t("admin.dashboard.ddns.extraDomainsError", {
            count: (ddnsStatus.value?.targets || []).filter(
              (target) =>
                !target.isPrimary && target.lastCheck.outcome === "error",
            ).length,
          })
        : t("admin.dashboard.ddns.extraDomainsCount"),
    icon: Globe,
  },
]);

const entryStatusCardTitle = computed(() =>
  showTunnelSection.value
    ? t("admin.dashboard.entry.entryAndTunnel")
    : t("admin.dashboard.entry.entryStatus"),
);

const entryStatusCardDescription = computed(() =>
  showTunnelSection.value
    ? t("admin.dashboard.entry.ddnsAndTunnelStatus")
    : t("admin.dashboard.entry.ddnsStatus"),
);

const tunnelCards = computed(() => [
  {
    key: "frp" as const,
    label: t("admin.dashboard.tunnel.frp"),
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
    >
      <div class="space-y-2 min-w-0">
        <div
          class="flex flex-wrap items-center gap-x-3 gap-y-2 text-sm text-muted-foreground"
        >
          <span>{{ t("admin.dashboard.labels.range") }}: {{ titleRangeText }}</span>
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
        <Tabs v-model="rangeKey" class="w-full sm:w-auto">
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
