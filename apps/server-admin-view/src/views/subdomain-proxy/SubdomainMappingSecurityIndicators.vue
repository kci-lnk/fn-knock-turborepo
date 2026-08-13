<script setup lang="ts">
import {
  BrickWall,
  Eye,
  PanelsTopLeft,
  Route as RouteIcon,
  ScanEye,
} from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import type { HostMappingSecurityIndicatorState } from "./model";
import type { SubdomainMappingStatusIndicatorsProps } from "./subdomain-mapping-status-contract";
import SubdomainMappingStatusTooltip from "./SubdomainMappingStatusTooltip.vue";

defineProps<{
  locationRulesCount: number;
  model: SubdomainMappingStatusIndicatorsProps;
  securityIndicators: HostMappingSecurityIndicatorState;
  shouldShowToolbarIndicator: boolean;
}>();
const { t } = useI18n();
</script>

<template>
  <SubdomainMappingStatusTooltip
    v-if="securityIndicators.waf"
    :model="model"
    tooltip="waf"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="
          t('admin.subdomainProxy.statusWafEnabledAria', {
            host: model.formatHost(model.mapping.host),
          })
        "
        @click="handleClick"
      >
        <BrickWall class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>{{ t("admin.subdomainProxy.statusWafEnabledTooltip") }}</p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-if="securityIndicators.visibility"
    :model="model"
    tooltip="visibility"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :class="
          securityIndicators.visibility === 'custom'
            ? 'text-primary hover:text-primary'
            : 'text-muted-foreground hover:text-foreground'
        "
        :aria-label="
          securityIndicators.visibility === 'custom'
            ? t('admin.subdomainProxy.statusVisibilityCustomAria', {
                host: model.formatHost(model.mapping.host),
                regions: securityIndicators.regionCount,
                cidrs: securityIndicators.customCidrCount,
              })
            : t('admin.subdomainProxy.statusVisibilityInheritAria', {
                host: model.formatHost(model.mapping.host),
              })
        "
        @click="handleClick"
      >
        <ScanEye
          v-if="securityIndicators.visibility === 'custom'"
          class="h-3.5 w-3.5"
        />
        <Eye v-else class="h-3.5 w-3.5" />
      </button>
    </template>
    <p v-if="securityIndicators.visibility === 'custom'">
      {{
        t("admin.subdomainProxy.statusVisibilityCustomTooltip", {
          regions: securityIndicators.regionCount,
          cidrs: securityIndicators.customCidrCount,
        })
      }}
    </p>
    <p v-else>
      {{ t("admin.subdomainProxy.statusVisibilityInheritTooltip") }}
    </p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-if="shouldShowToolbarIndicator"
    :model="model"
    tooltip="toolbar"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md text-muted-foreground transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="
          t('admin.subdomainProxy.statusToolbarEnabledAria', {
            host: model.formatHost(model.mapping.host),
          })
        "
        @click="handleClick"
      >
        <PanelsTopLeft class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>{{ t("admin.subdomainProxy.statusToolbarEnabledTooltip") }}</p>
  </SubdomainMappingStatusTooltip>

  <SubdomainMappingStatusTooltip
    v-if="locationRulesCount > 0"
    :model="model"
    tooltip="location-rules"
  >
    <template #trigger="{ handleClick }">
      <button
        type="button"
        class="inline-flex h-5 w-5 shrink-0 cursor-help items-center justify-center rounded-md transition-colors hover:bg-muted hover:text-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring/40"
        :aria-label="
          t('admin.subdomainProxy.locationRulesAria', {
            host: model.formatHost(model.mapping.host),
            count: locationRulesCount,
          })
        "
        @click="handleClick"
      >
        <RouteIcon class="h-3.5 w-3.5" />
      </button>
    </template>
    <p>
      {{
        t("admin.subdomainProxy.locationRulesCount", {
          count: locationRulesCount,
        })
      }}
    </p>
  </SubdomainMappingStatusTooltip>
</template>
