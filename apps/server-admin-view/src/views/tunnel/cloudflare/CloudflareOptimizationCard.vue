<script setup lang="ts">
import { Button } from "@/components/ui/button";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import CloudflareOptimizationDomains from "./CloudflareOptimizationDomains.vue";
import CloudflareOptimizationOverview from "./CloudflareOptimizationOverview.vue";
import CloudflareOptimizationScanResults from "./CloudflareOptimizationScanResults.vue";
import CloudflareOptimizationSourceSettings from "./CloudflareOptimizationSourceSettings.vue";
import CloudflareOptimizationTechnicalStatus from "./CloudflareOptimizationTechnicalStatus.vue";
import { useCloudflareOptimizationCardPresentation } from "./useCloudflareOptimizationCardPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const props = defineProps<{ controller: CloudflareTunnelController }>();
const {
  apiTokenConfigured,
  configLoaded,
  isLoadingManagedState,
  optimization,
  optimizationApplied,
  optimizationEnabled,
  t,
} = props.controller;
const presentation = useCloudflareOptimizationCardPresentation(
  props.controller,
);
</script>

<template>
  <ConfigCollapsibleCard
    v-if="
      apiTokenConfigured &&
      (optimizationEnabled ||
        optimizationApplied ||
        optimization?.capabilityProbe?.status === 'unsupported')
    "
    :title="t('admin.cloudflareTunnel.optimization.title')"
    :configured="optimizationApplied"
    :ready="configLoaded && !isLoadingManagedState"
    :edit-label="t('admin.cloudflareTunnel.managed.viewOrChange')"
    collapsed-content-class="min-h-[76px] flex flex-col items-start gap-3 sm:h-[40px] sm:flex-row sm:items-center sm:justify-between"
    summary-class="text-xs text-muted-foreground max-w-full whitespace-normal break-words sm:truncate"
    expanded-content-class="p-0 sm:p-0"
  >
    <template #summary>
      {{
        optimizationApplied
          ? optimization?.fallbackActive
            ? t("admin.cloudflareTunnel.optimization.summaryFallback")
            : t("admin.cloudflareTunnel.optimization.summaryActive", {
                count: presentation.optimizedDomainCount,
                total: presentation.optimizationManagedDomainCount,
                ip: optimization?.selected?.ip || "-",
              })
          : t("admin.cloudflareTunnel.optimization.summaryNotApplied")
      }}
    </template>

    <template #default>
      <div class="space-y-5 p-4 sm:p-6">
        <CloudflareOptimizationOverview
          :controller="controller"
          :presentation="presentation"
        >
          <template #source-settings>
            <CloudflareOptimizationSourceSettings :controller="controller" />
          </template>
        </CloudflareOptimizationOverview>
        <CloudflareOptimizationScanResults :controller="controller" />
        <CloudflareOptimizationDomains :controller="controller" />
        <CloudflareOptimizationTechnicalStatus
          :controller="controller"
          :presentation="presentation"
        />
      </div>
    </template>

    <template #actions="{ collapse }">
      <div
        class="flex justify-end rounded-b-lg border-t bg-muted/30 p-4 sm:px-6"
      >
        <Button variant="outline" @click="collapse">
          {{ t("admin.cloudflareTunnel.collapse") }}
        </Button>
      </div>
    </template>
  </ConfigCollapsibleCard>
</template>
