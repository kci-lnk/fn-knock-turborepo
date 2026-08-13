<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Activity,
  ArrowUpFromLine,
  Gauge,
  TriangleAlert,
  UsersRound,
} from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import TimeSeriesChart from "@/components/charts/TimeSeriesChart.vue";
import type {
  RequestAnalyticsMetricKey,
  RequestAnalyticsPageModel,
} from "./useRequestAnalyticsPage";

defineProps<{ model: RequestAnalyticsPageModel }>();
const { t } = useI18n();
const metricIcons: Record<RequestAnalyticsMetricKey, typeof Activity> = {
  requests: Activity,
  clients: UsersRound,
  errors: TriangleAlert,
  p95: Gauge,
  traffic: ArrowUpFromLine,
};
</script>

<template>
  <div class="grid grid-cols-2 gap-2.5 sm:gap-3 lg:grid-cols-3 xl:grid-cols-5">
    <Card
      v-for="metric in model.metricCards"
      :key="metric.key"
      class="min-w-0 shadow-none"
      :class="{ 'col-span-2 lg:col-span-1': metric.key === 'traffic' }"
    >
      <CardContent class="p-3 sm:p-4">
        <div class="flex items-start justify-between gap-2">
          <p class="text-xs text-muted-foreground">{{ metric.label }}</p>
          <component
            :is="metricIcons[metric.key]"
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
      <CardTitle class="text-base">
        {{ t("admin.requestAnalysis.chart.title") }}
      </CardTitle>
      <CardDescription class="text-xs sm:text-sm">
        {{ t("admin.requestAnalysis.chart.description") }}
      </CardDescription>
    </CardHeader>
    <CardContent
      class="h-[280px] p-2 sm:h-[380px] sm:p-4"
      role="img"
      :aria-label="t('admin.requestAnalysis.chart.ariaLabel')"
    >
      <TimeSeriesChart
        :series="model.hasRequests ? model.chartSeries : []"
        :time-formatter="model.formatChartTime"
        :empty-text="t('admin.requestAnalysis.empty')"
        :min-height="220"
      />
    </CardContent>
  </Card>

  <Alert
    v-if="(model.data?.quality.invalid_entries || 0) > 0"
    class="border-amber-500/25 bg-amber-500/5 text-foreground"
  >
    <TriangleAlert class="h-4 w-4 text-amber-600" />
    <p class="text-sm text-muted-foreground">
      {{
        t("admin.requestAnalysis.qualityWarning", {
          count: model.data?.quality.invalid_entries || 0,
        })
      }}
    </p>
  </Alert>
</template>
