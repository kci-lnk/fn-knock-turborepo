<script setup lang="ts">
import { CircleOff, Clock } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import { Badge } from "@/components/ui/badge";
import type { SubdomainMappingStatusIndicatorsProps } from "./subdomain-mapping-status-contract";
import SubdomainMappingStatusTooltip from "./SubdomainMappingStatusTooltip.vue";

defineProps<{ model: SubdomainMappingStatusIndicatorsProps }>();
const { t } = useI18n();
</script>

<template>
  <Badge
    v-if="model.availabilityState === 'disabled'"
    variant="outline"
    class="gap-1 border-muted-foreground/30 px-1.5 text-muted-foreground"
  >
    <CircleOff class="h-3 w-3" />
    {{ t("admin.subdomainProxy.disabledBadge") }}
  </Badge>

  <SubdomainMappingStatusTooltip
    v-else-if="model.availabilityState === 'scheduled_closed'"
    :model="model"
    tooltip="availability"
  >
    <template #trigger="{ handleClick }">
      <Badge
        as="button"
        type="button"
        variant="outline"
        class="inline-flex h-6 w-6 cursor-help items-center justify-center rounded-full border-amber-500/35 bg-amber-500/5 p-0 text-amber-700 transition-colors hover:bg-amber-500/10 dark:text-amber-300"
        :aria-label="t('admin.subdomainProxy.unavailableBadge')"
        @click="handleClick"
      >
        <Clock class="h-3 w-3" />
      </Badge>
    </template>
    <p>
      {{
        t("admin.subdomainProxy.scheduleClosedTooltip", {
          window: model.availabilityWindow,
        })
      }}
    </p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-else-if="model.availabilityState === 'scheduled_open'"
    :model="model"
    tooltip="availability"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="t('admin.subdomainProxy.scheduleOpenAria')"
        @click="handleClick"
      >
        <Clock class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>
      {{
        t("admin.subdomainProxy.scheduleOpenTooltip", {
          window: model.availabilityWindow,
        })
      }}
    </p>
  </SubdomainMappingStatusTooltip>
</template>
