<script setup lang="ts">
import type { CloudflareOptimizationCardPresentation } from "./useCloudflareOptimizationCardPresentation";
import type { CloudflareTunnelController } from "./useCloudflareTunnelController";

const props = defineProps<{
  controller: CloudflareTunnelController;
  presentation: CloudflareOptimizationCardPresentation;
}>();
const { optimization, t } = props.controller;
</script>

<template>
  <details class="rounded-lg border bg-muted/20">
    <summary class="cursor-pointer list-none px-4 py-3 text-sm font-medium">
      {{ t("admin.cloudflareTunnel.optimization.technicalStatus") }}
    </summary>
    <div class="grid gap-3 border-t p-4 text-sm sm:grid-cols-2">
      <div>
        <div class="text-xs text-muted-foreground">
          {{ t("admin.cloudflareTunnel.optimization.capabilityProbe") }}
        </div>
        <div class="mt-1">{{ presentation.capabilityProbeMessage }}</div>
      </div>
      <div>
        <div class="text-xs text-muted-foreground">
          {{ t("admin.cloudflareTunnel.optimization.lastHealth") }}
        </div>
        <div class="mt-1">
          {{ presentation.formatDate(optimization?.schedule.lastHealthAt) }}
        </div>
      </div>
      <div v-if="optimization?.schedule.lastSwitchReason">
        <div class="text-xs text-muted-foreground">
          {{ t("admin.cloudflareTunnel.optimization.lastSwitchReason") }}
        </div>
        <div class="mt-1">
          {{
            presentation.switchReasonLabel(
              optimization.schedule.lastSwitchReason,
            )
          }}
        </div>
      </div>
    </div>
  </details>
</template>
