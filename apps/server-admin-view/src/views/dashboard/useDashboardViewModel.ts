import { computed, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  ArrowDownLeft,
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
import { type DashboardStats, type TrafficStats } from "@/lib/api/dashboard";
import { type DDNSStatusPayload } from "@/lib/api/ddns";
import { type ThreatOverview } from "@/types";
import type { TimeSeriesChartSeries } from "@/components/charts/TimeSeriesChart.vue";
import { buildDDNSTimestampTooltipLines } from "@/lib/ddns-time";
import { isCloudflaredTunnelAvailable } from "@/lib/reverse-proxy-submode";
import { useConfigStore } from "@/store/config";
import type { TunnelStatus } from "./useDashboardTunnelStatus";

const ddnsUpdateScopeLabelKeys = {
  dual_stack: "admin.dashboard.ddns.updateScopes.dualStack",
  ipv6_only: "admin.dashboard.ddns.updateScopes.ipv6Only",
  ipv4_only: "admin.dashboard.ddns.updateScopes.ipv4Only",
} as const;

const trafficSeriesLabelKeys: Record<string, string> = {
  "\u5165\u7ad9": "admin.dashboard.traffic.ingressSeries",
  "\u51fa\u7ad9": "admin.dashboard.traffic.egressSeries",
};

const metricIconTones = {
  liveIngress: { color: "#0f766e" },
  liveEgress: { color: "#c2410c" },
  totalIngress: { color: "#0f766e" },
  totalEgress: { color: "#c2410c" },
} as const;

export const DASHBOARD_TRAFFIC_COLORS = {
  ingress: "#0f766e",
  egress: "#c2410c",
} as const;

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

export const useDashboardViewModel = ({
  activeRangeSec,
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
}: {
  activeRangeSec: () => number;
  cfStatus: Ref<TunnelStatus | null>;
  ddnsStatus: Ref<DDNSStatusPayload | null>;
  defaultTunnel: Ref<"frp" | "cloudflared">;
  frpStatus: Ref<TunnelStatus | null>;
  realtimeInBps: Ref<number | null>;
  realtimeOutBps: Ref<number | null>;
  realtimeStats: Ref<TrafficStats | null>;
  showTunnelSection: Readonly<Ref<boolean>>;
  stats: Ref<DashboardStats | null>;
  threatOverview: Ref<ThreatOverview | null>;
}) => {
  const configStore = useConfigStore();
  const { locale, t } = useI18n();

  const formatBytes = (bytes: number | null | undefined) => {
    const value = Number(bytes ?? 0);
    if (!Number.isFinite(value) || value <= 0) return "0 B";
    const units = ["B", "KB", "MB", "GB", "TB"] as const;
    const exponent = Math.max(
      0,
      Math.min(units.length - 1, Math.floor(Math.log(value) / Math.log(1024))),
    );
    const normalized = value / 1024 ** exponent;
    const digits =
      exponent === 0 ? 0 : normalized >= 100 ? 0 : normalized >= 10 ? 1 : 2;
    return `${normalized.toFixed(digits)} ${units[exponent]}`;
  };

  const formatBps = (bps: number | null | undefined) =>
    `${formatBytes(bps)} /s`;

  const formatNumber = (value: number | null | undefined, fallback = "-") => {
    if (value === null || value === undefined) return fallback;
    const normalized = Number(value);
    if (!Number.isFinite(normalized)) return fallback;
    return new Intl.NumberFormat(String(locale.value)).format(
      Math.round(normalized),
    );
  };

  const translateTrafficSeriesName = (name: unknown) => {
    const value = String(name ?? "");
    const key = trafficSeriesLabelKeys[value];
    return key ? t(key) : value;
  };

  const onlineNow = computed(
    () => realtimeStats.value?.active_conns ?? stats.value?.now?.online ?? null,
  );

  const trafficSeries = computed<TimeSeriesChartSeries[]>(() => {
    const base = (stats.value?.traffic.echarts ?? {}) as any;
    const colors = [
      DASHBOARD_TRAFFIC_COLORS.ingress,
      DASHBOARD_TRAFFIC_COLORS.egress,
    ];

    return (Array.isArray(base?.series) ? base.series : []).map(
      (series: any, index: number) => {
        const color = colors[index % colors.length] ?? "#0f766e";
        return {
          name: translateTrafficSeriesName(series?.name),
          color,
          fill: `${color}14`,
          data: normalizeSeriesData(series?.data),
        };
      },
    );
  });

  const threatSeries = computed<TimeSeriesChartSeries[]>(() => [
    {
      name: t("admin.dashboard.security.failedLogins"),
      color: "#525252",
      fill: "rgba(82, 82, 82, 0.08)",
      data: threatOverview.value?.series.failedLogins ?? [],
    },
    {
      name: t("admin.dashboard.security.scanners"),
      color: "#991b1b",
      fill: "rgba(153, 27, 27, 0.08)",
      data: threatOverview.value?.series.blockedScanners ?? [],
    },
    {
      name: "WAF",
      color: "#b45309",
      fill: "rgba(180, 83, 9, 0.08)",
      data: threatOverview.value?.series.wafEvents ?? [],
    },
  ]);

  const titleRangeText = computed(() => {
    const seconds = stats.value?.rangeSec ?? activeRangeSec();
    if (seconds < 3600) {
      return t("admin.dashboard.duration.minutes", {
        count: Math.round(seconds / 60),
      });
    }
    if (seconds < 24 * 3600) {
      return t("admin.dashboard.duration.hours", {
        count: Math.round(seconds / 3600),
      });
    }
    return t("admin.dashboard.duration.days", {
      count: Math.round(seconds / 86400),
    });
  });

  const liveMetricCards = computed(() => [
    {
      label: t("admin.dashboard.metrics.liveIngress"),
      value:
        realtimeInBps.value === null ? "-" : formatBps(realtimeInBps.value),
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

  const ddnsState = computed(() => ({
    active: Boolean(ddnsStatus.value?.enabled),
    label: ddnsStatus.value?.enabled
      ? t("admin.dashboard.ddns.activeSync")
      : t("admin.dashboard.ddns.paused"),
  }));

  const getDdnsTimestampLabels = () => ({
    lastSuccessfulUpdate: t("admin.ddns.lastSuccessfulUpdate"),
    lastCheck: t("admin.ddns.lastCheck"),
    never: t("admin.ddns.never"),
  });

  const ddnsCards = computed(() => [
    {
      label: t("admin.dashboard.ddns.provider"),
      value:
        ddnsStatus.value?.provider || t("admin.dashboard.ddns.notConfigured"),
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

  const showCloudflaredTunnel = computed(
    () =>
      configStore.canUseCloudflared &&
      isCloudflaredTunnelAvailable(configStore.config),
  );

  const tunnelCards = computed(() => [
    ...(configStore.canUseFrpc
      ? [
          {
            key: "frp" as const,
            label: t("admin.dashboard.tunnel.frp"),
            status: frpStatus.value,
            isDefault: defaultTunnel.value === "frp",
          },
        ]
      : []),
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

  return {
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
  };
};
