import { computed, reactive } from "vue";
import { useI18n } from "vue-i18n";
import type { TimeSeriesChartSeries } from "@/components/charts/TimeSeriesChart.vue";
import type { GatewayLogAnalyticsPayload } from "@/types";
import {
  analyticsCountryLabel,
  analyticsDimensionLabel,
  analyticsRegionLabel,
  analyticsTimestampOffsetMinutes,
  formatAnalyticsBytes,
  formatAnalyticsDuration,
  formatAnalyticsNumber,
  formatAnalyticsPercent,
  mapAnalyticsBuckets,
} from "./model";
import { useGatewayRequestAnalytics } from "./useGatewayRequestAnalytics";

export type RequestAnalyticsMetricKey =
  | "requests"
  | "clients"
  | "errors"
  | "p95"
  | "traffic";

type AnalyticsDimensionKey = keyof GatewayLogAnalyticsPayload["dimensions"];

export const useRequestAnalyticsPage = () => {
  const { locale, t } = useI18n();
  const resource = useGatewayRequestAnalytics();
  const localeCode = computed(() => String(locale.value));
  const hasRequests = computed(
    () => (resource.data.value?.summary.requests || 0) > 0,
  );

  const metricCards = computed<
    Array<{ key: RequestAnalyticsMetricKey; label: string; value: string }>
  >(() => {
    const summary = resource.data.value?.summary;
    return [
      {
        key: "requests",
        label: t("admin.requestAnalysis.metrics.requests"),
        value: formatAnalyticsNumber(
          summary?.requests || 0,
          localeCode.value,
        ),
      },
      {
        key: "clients",
        label: t("admin.requestAnalysis.metrics.uniqueClients"),
        value: formatAnalyticsNumber(
          summary?.unique_clients || 0,
          localeCode.value,
        ),
      },
      {
        key: "errors",
        label: t("admin.requestAnalysis.metrics.serverErrorRate"),
        value: formatAnalyticsPercent(
          summary?.server_error_rate || 0,
          localeCode.value,
        ),
      },
      {
        key: "p95",
        label: t("admin.requestAnalysis.metrics.p95Duration"),
        value: formatAnalyticsDuration(
          summary?.p95_duration_ms || 0,
          localeCode.value,
        ),
      },
      {
        key: "traffic",
        label: t("admin.requestAnalysis.metrics.bytesOut"),
        value: formatAnalyticsBytes(
          summary?.bytes_out || 0,
          localeCode.value,
        ),
      },
    ];
  });

  const chartSeries = computed<TimeSeriesChartSeries[]>(() => [
    {
      name: t("admin.requestAnalysis.chart.requests"),
      color: "#2563eb",
      fill: "rgba(37, 99, 235, 0.12)",
      data:
        resource.data.value?.series.map((point) => [
          point.bucket_start,
          point.requests,
        ]) || [],
    },
    {
      name: t("admin.requestAnalysis.chart.clientErrors"),
      color: "#d97706",
      fill: "rgba(217, 119, 6, 0.04)",
      data:
        resource.data.value?.series.map((point) => [
          point.bucket_start,
          point.client_errors,
        ]) || [],
    },
    {
      name: t("admin.requestAnalysis.chart.serverErrors"),
      color: "#dc2626",
      fill: "rgba(220, 38, 38, 0.04)",
      data:
        resource.data.value?.series.map((point) => [
          point.bucket_start,
          point.server_errors,
        ]) || [],
    },
  ]);
  const chartTimeOffsets = computed(() =>
    (resource.data.value?.series || [])
      .map((point) => ({
        timestamp: Date.parse(point.bucket_start),
        offset: analyticsTimestampOffsetMinutes(point.bucket_start),
      }))
      .filter((point) => Number.isFinite(point.timestamp)),
  );
  const gatewayChartTimestamp = (value: number) => {
    let offset = 0;
    let distance = Number.POSITIVE_INFINITY;
    for (const point of chartTimeOffsets.value) {
      const nextDistance = Math.abs(point.timestamp - value);
      if (nextDistance < distance) {
        offset = point.offset;
        distance = nextDistance;
      }
    }
    return value + offset * 60_000;
  };
  const formatChartTime = (value: number) => {
    const granularity = resource.data.value?.range.granularity;
    return new Intl.DateTimeFormat(localeCode.value, {
      month: "short",
      day: "numeric",
      ...(granularity === "day"
        ? {}
        : { hour: "2-digit", minute: "2-digit" }),
      timeZone: "UTC",
    }).format(new Date(gatewayChartTimestamp(value)));
  };

  const dimensionItems = (key: AnalyticsDimensionKey) =>
    mapAnalyticsBuckets(
      resource.data.value?.dimensions[key] || [],
      (value) => analyticsDimensionLabel(value, t),
    );
  const targetTabs = computed(() => [
    {
      key: "paths",
      label: t("admin.requestAnalysis.tabs.paths"),
      items: dimensionItems("paths"),
    },
    {
      key: "routes",
      label: t("admin.requestAnalysis.tabs.routes"),
      items: dimensionItems("routes"),
    },
    {
      key: "hosts",
      label: t("admin.requestAnalysis.tabs.hosts"),
      items: dimensionItems("hosts"),
    },
    {
      key: "upstreams",
      label: t("admin.requestAnalysis.tabs.upstreams"),
      items: dimensionItems("upstreams"),
    },
  ]);
  const sourceTabs = computed(() => [
    {
      key: "referrers",
      label: t("admin.requestAnalysis.tabs.referrers"),
      items: dimensionItems("referrers"),
    },
    {
      key: "utm_sources",
      label: t("admin.requestAnalysis.tabs.utmSources"),
      items: dimensionItems("utm_sources"),
    },
    {
      key: "utm_mediums",
      label: t("admin.requestAnalysis.tabs.utmMediums"),
      items: dimensionItems("utm_mediums"),
    },
    {
      key: "utm_campaigns",
      label: t("admin.requestAnalysis.tabs.utmCampaigns"),
      items: dimensionItems("utm_campaigns"),
    },
  ]);

  const geoFooter = (
    status: "complete" | "resolving" | "partial",
    resolved: number,
    total: number,
    coverage: number,
  ) => {
    if (status === "resolving") {
      return t("admin.requestAnalysis.geo.resolving", { resolved, total });
    }
    return t(
      status === "complete"
        ? "admin.requestAnalysis.geo.complete"
        : "admin.requestAnalysis.geo.partial",
      { coverage: formatAnalyticsPercent(coverage, localeCode.value) },
    );
  };
  const geoTabs = computed(() => {
    const geo = resource.data.value?.geo;
    return [
      {
        key: "countries",
        label: t("admin.requestAnalysis.tabs.countries"),
        metricLabel: t("admin.requestAnalysis.metrics.uniqueClients"),
        items: mapAnalyticsBuckets(geo?.items || [], (value) =>
          analyticsCountryLabel(value, localeCode.value, t),
        ),
        footer: geo
          ? geoFooter(
              geo.status,
              geo.resolved_clients,
              geo.total_clients,
              geo.coverage,
            )
          : "",
      },
      {
        key: "regions",
        label: t("admin.requestAnalysis.tabs.regions"),
        metricLabel: t("admin.requestAnalysis.metrics.uniqueClients"),
        items: (geo?.regions || []).map((item) => ({
          ...item,
          label: analyticsRegionLabel(item, localeCode.value, t),
        })),
        footer: geo
          ? geoFooter(
              geo.region_status,
              geo.resolved_region_clients,
              geo.total_clients,
              geo.region_coverage,
            )
          : "",
      },
    ];
  });
  const clientTabs = computed(() => [
    {
      key: "devices",
      label: t("admin.requestAnalysis.tabs.devices"),
      items: dimensionItems("devices"),
    },
    {
      key: "browsers",
      label: t("admin.requestAnalysis.tabs.browsers"),
      items: dimensionItems("browsers"),
    },
    {
      key: "operating_systems",
      label: t("admin.requestAnalysis.tabs.operatingSystems"),
      items: dimensionItems("operating_systems"),
    },
  ]);
  const responseTabs = computed(() => [
    {
      key: "statuses",
      label: t("admin.requestAnalysis.tabs.statuses"),
      items: dimensionItems("statuses"),
    },
    {
      key: "methods",
      label: t("admin.requestAnalysis.tabs.methods"),
      items: dimensionItems("methods"),
    },
    {
      key: "latency_bands",
      label: t("admin.requestAnalysis.tabs.latency"),
      items: dimensionItems("latency_bands"),
    },
  ]);
  const securityTabs = computed(() => [
    {
      key: "auth_decisions",
      label: t("admin.requestAnalysis.tabs.authDecisions"),
      items: dimensionItems("auth_decisions"),
    },
    {
      key: "waf_actions",
      label: t("admin.requestAnalysis.tabs.wafActions"),
      items: dimensionItems("waf_actions"),
    },
  ]);

  return reactive({
    ...resource,
    chartSeries,
    clientTabs,
    formatChartTime,
    geoTabs,
    hasRequests,
    metricCards,
    responseTabs,
    securityTabs,
    sourceTabs,
    targetTabs,
  });
};

export type RequestAnalyticsPageModel = ReturnType<
  typeof useRequestAnalyticsPage
>;
