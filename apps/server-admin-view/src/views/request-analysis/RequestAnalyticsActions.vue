<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import RefreshButton from "@/components/RefreshButton.vue";
import { REQUEST_ANALYTICS_RANGE_OPTIONS } from "./model";
import type { RequestAnalyticsPageModel } from "./useRequestAnalyticsPage";

defineProps<{ model: RequestAnalyticsPageModel }>();
const { t } = useI18n();
</script>

<template>
  <Teleport defer to="#request-analysis-analytics-actions">
    <div
      class="grid w-full grid-cols-[minmax(0,1fr)_auto] items-center gap-2 sm:flex sm:w-auto"
    >
      <Tabs
        :model-value="model.rangeKey"
        class="min-w-0"
        @update:model-value="model.selectRange"
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
        :loading="model.loading"
        :disabled="model.loading"
        class="shrink-0 px-2.5 [&_span]:hidden [&_svg]:mr-0 sm:px-3 sm:[&_span]:inline sm:[&_svg]:mr-1.5"
        @click="model.refresh"
      />
    </div>
  </Teleport>
</template>
