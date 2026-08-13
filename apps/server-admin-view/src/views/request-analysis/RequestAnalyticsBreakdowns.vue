<script setup lang="ts">
import { useI18n } from "vue-i18n";
import RefreshButton from "@/components/RefreshButton.vue";
import AnalyticsBreakdownCard from "./AnalyticsBreakdownCard.vue";
import type { RequestAnalyticsPageModel } from "./useRequestAnalyticsPage";

defineProps<{ model: RequestAnalyticsPageModel }>();
const { t } = useI18n();
</script>

<template>
  <div class="grid gap-3 sm:gap-4 xl:grid-cols-2">
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.targets')"
      :tabs="model.targetTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
    />
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.sources')"
      :tabs="model.sourceTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
    />
  </div>

  <div class="grid gap-3 sm:gap-4 md:grid-cols-2 xl:grid-cols-4">
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.geo')"
      :tabs="model.geoTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.uniqueClients')"
    >
      <template #action>
        <RefreshButton
          icon-only
          size="icon"
          :label="t('admin.requestAnalysis.geo.refresh')"
          :loading="model.geoRefreshing"
          :disabled="model.geoRefreshing || !model.data?.summary.unique_clients"
          @click="model.refreshGeo"
        />
      </template>
    </AnalyticsBreakdownCard>
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.clients')"
      :tabs="model.clientTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
    />
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.responses')"
      :tabs="model.responseTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
    />
    <AnalyticsBreakdownCard
      :title="t('admin.requestAnalysis.cards.security')"
      :tabs="model.securityTabs"
      :empty-text="t('admin.requestAnalysis.empty')"
      :default-metric-label="t('admin.requestAnalysis.metrics.requests')"
    />
  </div>
</template>
