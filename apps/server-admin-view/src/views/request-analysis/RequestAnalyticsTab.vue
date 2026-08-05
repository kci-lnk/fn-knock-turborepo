<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import {
  Activity,
  ArrowUpFromLine,
  Gauge,
  TriangleAlert,
  UsersRound,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Skeleton } from "@/components/ui/skeleton";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import RefreshButton from "@/components/RefreshButton.vue";
import TimeSeriesChart, {
  type TimeSeriesChartSeries,
} from "@/components/charts/TimeSeriesChart.vue";
import AnalyticsBreakdownCard from "./AnalyticsBreakdownCard.vue";
import type { GatewayLogAnalyticsPayload } from "@/types";
import {
  REQUEST_ANALYTICS_RANGE_OPTIONS,
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

const { locale, t } = useI18n();
const {
  data,
  geoRefreshing,
  loadFailed,
  loading,
  rangeKey,
  refresh,
  refreshGeo,
  selectRange,
} = useGatewayRequestAnalytics();

const localeCode = computed(() => String(locale.value));
const hasRequests = computed(() => (data.value?.summary.requests || 0) > 0);

const metricCards = computed(() => {
  const summary = data.value?.summary;
  return [
    {
      key: "requests",
      label: t("admin.requestAnalysis.metrics.requests"),
      value: formatAnalyticsNumber(summary?.requests || 0, localeCode.value),
      icon: Activity,
    },
    {
      key: "clients",
      label: t("admin.requestAnalysis.metrics.uniqueClients"),
      value: formatAnalyticsNumber(
        summary?.unique_clients || 0,
        localeCode.value,
      ),
      icon: UsersRound,
    },
    {
      key: "errors",
      label: t("admin.requestAnalysis.metrics.serverErrorRate"),
      value: formatAnalyticsPercent(
        summary?.server_error_rate || 0,
        localeCode.value,
      ),
      icon: TriangleAlert,
    },
    {
      key: "p95",
      label: t("admin.requestAnalysis.metrics.p95Duration"),
      value: formatAnalyticsDuration(
        summary?.p95_duration_ms || 0,
        localeCode.value,
      ),
      icon: Gauge,
    },
    {
      key: "traffic",
      label: t("admin.requestAnalysis.metrics.bytesOut"),
      value: formatAnalyticsBytes(summary?.bytes_out || 0, localeCode.value),
      icon: ArrowUpFromLine,
    },
  ];
});

const chartSeries = computed<TimeSeriesChartSeries[]>(() => [
  {
    name: t("admin.requestAnalysis.chart.requests"),
    color: "#2563eb",
    fill: "rgba(37, 99, 235, 0.12)",
    data:
      data.value?.series.map((point) => [point.bucket_start, point.requests]) ||
      [],
  },
  {
    name: t("admin.requestAnalysis.chart.clientErrors"),
    color: "#d97706",
    fill: "rgba(217, 119, 6, 0.04)",
    data:
      data.value?.series.map((point) => [
        point.bucket_start,
        point.client_errors,
      ]) || [],
  },
  {
    name: t("admin.requestAnalysis.chart.serverErrors"),
    color: "#dc2626",
    fill: "rgba(220, 38, 38, 0.04)",
    data:
      data.value?.series.map((point) => [
        point.bucket_start,
        point.server_errors,
      ]) || [],
  },
]);

const chartTimeOffsets = computed(() =>
  (data.value?.series || [])
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
  const granularity = data.value?.range.granularity;
  return new Intl.DateTimeFormat(localeCode.value, {
    month: "short",
    day: "numeric",
    ...(granularity === "day" ? {} : { hour: "2-digit", minute: "2-digit" }),
    timeZone: "UTC",
  }).format(new Date(gatewayChartTimestamp(value)));
};

type AnalyticsDimensionKey = keyof GatewayLogAnalyticsPayload["dimensions"];

const dimensionItems = (key: AnalyticsDimensionKey) =>
  mapAnalyticsBuckets(data.value?.dimensions[key] || [], (value) =>
    analyticsDimensionLabel(value, t),
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
  const geo = data.value?.geo;
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
</script>

<template>
  <div class="space-y-3 sm:space-y-4">
    <Teleport defer to="#request-analysis-analytics-actions">
      <div
        class="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-2 sm:flex sm:w-auto"
      >
        <Tabs
          :model-value="rangeKey"
          class="min-w-0"
          @update:model-value="selectRange"
        >
          <TabsList
            class="grid w-full grid-cols-3 sm:w-auto"
            :aria-label="t('admin.requestAnalysis.ranges.label')"
          >
            <TabsTrigger
              v-for="option in REQUEST_ANALYTICS_RANGE_OPTIONS"
              :key="option.key"
              :value="option.key"
              class="px-3 text-xs sm:text-sm"
            >
              {{ t(option.labelKey) }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
        <RefreshButton
          :loading="loading"
          :disabled="loading"
          class="shrink-0 px-2.5 [&_span]:hidden [&_svg]:mr-0 sm:px-3 sm:[&_span]:inline sm:[&_svg]:mr-1.5"
          @click="refresh"
        />
      </div>
    </Teleport>

    <div v-if="loading && !data" class="space-y-4">
      <div class="grid grid-cols-2 gap-2.5 sm:gap-3 lg:grid-cols-3 xl:grid-cols-5">
        <Skeleton v-for="index in 5" :key="index" class="h-32 rounded-xl" />
      </div>
      <Skeleton class="h-[360px] rounded-xl" />
      <div class="grid gap-4 xl:grid-cols-2">
        <Skeleton class="h-80 rounded-xl" />
        <Skeleton class="h-80 rounded-xl" />
      </div>
    </div>

    <Alert
      v-else-if="loadFailed && !data"
      class="flex flex-col items-start gap-3 border-destructive/30 bg-destructive/5 text-foreground sm:flex-row sm:items-center sm:justify-between"
    >
      <div class="flex items-start gap-3">
        <TriangleAlert class="mt-0.5 h-4 w-4 shrink-0 text-destructive" />
        <div>
          <p class="text-sm font-medium">
            {{ t("admin.requestAnalysis.loadFailed") }}
          </p>
          <p class="mt-1 text-xs text-muted-foreground">
            {{ t("admin.requestAnalysis.loadFailedDescription") }}
          </p>
        </div>
      </div>
      <Button type="button" variant="outline" size="sm" @click="refresh">
        {{ t("admin.requestAnalysis.retry") }}
      </Button>
    </Alert>

    <template v-else>
      <div class="grid grid-cols-2 gap-2.5 sm:gap-3 lg:grid-cols-3 xl:grid-cols-5">
        <Card
          v-for="metric in metricCards"
          :key="metric.key"
          class="min-w-0 shadow-none"
          :class="{ 'col-span-2 lg:col-span-1': metric.key === 'traffic' }"
        >
          <CardContent class="p-3 sm:p-4">
            <div class="flex items-start justify-between gap-2">
              <p class="text-xs text-muted-foreground">{{ metric.label }}</p>
              <component
                :is="metric.icon"
                class="h-4 w-4 shrink-0 text-muted-foreground"
                aria-hidden="true"
              />
            </div>
            <p
              class="mt-2.5 truncate text-xl font-semibold tracking-tight tabular-nums sm:mt-3 sm:text-2xl"
              :title="metric.value"
            >
              {{ metric.value }}
            </p>
          </CardContent>
        </Card>
      </div>

      <Card class="overflow-hidden shadow-none">
        <CardHeader class="border-b px-4 py-3 sm:px-6 sm:py-4">
          <CardTitle class="text-base">{{
            t("admin.requestAnalysis.chart.title")
          }}</CardTitle>
          <CardDescription class="text-xs sm:text-sm">{{
            t("admin.requestAnalysis.chart.description")
          }}</CardDescription>
        </CardHeader>
        <CardContent
          class="h-[280px] p-2 sm:h-[380px] sm:p-4"
          role="img"
          :aria-label="t('admin.requestAnalysis.chart.ariaLabel')"
        >
          <TimeSeriesChart
            :series="hasRequests ? chartSeries : []"
            :time-formatter="formatChartTime"
            :empty-text="t('admin.requestAnalysis.empty')"
            :min-height="220"
          />
        </CardContent>
      </Card>

      <Alert
        v-if="(data?.quality.invalid_entries || 0) > 0"
        class="border-amber-500/25 bg-amber-500/5 text-foreground"
      >
        <TriangleAlert class="h-4 w-4 text-amber-600" />
        <p class="text-sm text-muted-foreground">
          {{
            t("admin.requestAnalysis.qualityWarning", {
              count: data?.quality.invalid_entries || 0,
            })
          }}
        </p>
      </Alert>

      <div class="grid gap-3 sm:gap-4 xl:grid-cols-2">
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.targets')"
          :tabs="targetTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
        />
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.sources')"
          :tabs="sourceTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
        />
      </div>

      <div class="grid gap-3 sm:gap-4 md:grid-cols-2 xl:grid-cols-4">
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.geo')"
          :tabs="geoTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="
            t('admin.requestAnalysis.metrics.uniqueClients')
          "
        >
          <template #action>
            <RefreshButton
              icon-only
              size="icon"
              :label="t('admin.requestAnalysis.geo.refresh')"
              :loading="geoRefreshing"
              :disabled="geoRefreshing || !data?.summary.unique_clients"
              @click="refreshGeo"
            />
          </template>
        </AnalyticsBreakdownCard>
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.clients')"
          :tabs="clientTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
        />
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.responses')"
          :tabs="responseTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
        />
        <AnalyticsBreakdownCard
          :title="t('admin.requestAnalysis.cards.security')"
          :tabs="securityTabs"
          :empty-text="t('admin.requestAnalysis.empty')"
          :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
        />
      </div>
    </template>
  </div>
</template>
