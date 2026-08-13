<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Ban } from "lucide-vue-next";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import ThreatOverviewCard from "@admin-shared/components/common/ThreatOverviewCard.vue";
import TimeSeriesChart from "@/components/charts/TimeSeriesChart.vue";
import type { IpBlacklistPageController } from "./useIpBlacklistPage";

const props = defineProps<{ controller: IpBlacklistPageController }>();
const { t } = useI18n();
const {
  blockedPerHour,
  blockedTrendSeries,
  formatNumber,
  formatRate,
  isThreatLoading,
  rangeKey,
  ranges,
  threatOverview,
  titleRangeText,
} = props.controller;
</script>

<template>
  <ConfigCollapsibleCard
    :title="t('admin.sessions.ipBlacklist.chartTitle')"
    :configured="true"
    :edit-label="t('admin.sessions.ipBlacklist.expandChart')"
    summary-class="text-xs text-muted-foreground"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        t("admin.sessions.ipBlacklist.chartSummary", {
          range: titleRangeText,
          count: formatNumber(threatOverview?.totals?.blockedScanners),
        })
      }}
    </template>

    <template #default>
      <ThreatOverviewCard
        v-model:range-key="rangeKey"
        :title="t('admin.sessions.ipBlacklist.overviewTitle')"
        :description="t('admin.sessions.ipBlacklist.overviewDescription')"
        :ranges="ranges"
        :is-loading="isThreatLoading"
        :title-range-text="titleRangeText"
        :primary-label="t('admin.sessions.ipBlacklist.primaryLabel')"
        :primary-value="formatNumber(threatOverview?.totals?.blockedScanners)"
        :primary-hint="t('admin.sessions.ipBlacklist.primaryHint')"
        :secondary-label="t('admin.sessions.ipBlacklist.secondaryLabel')"
        :secondary-value="formatRate(blockedPerHour)"
        :secondary-hint="t('admin.sessions.ipBlacklist.secondaryHint')"
        :icon="Ban"
      >
        <template #chart>
          <TimeSeriesChart
            :series="blockedTrendSeries"
            :value-formatter="(value) => formatNumber(value)"
            class="h-full w-full"
          />
        </template>
      </ThreatOverviewCard>
    </template>
  </ConfigCollapsibleCard>
</template>
