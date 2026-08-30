<script setup lang="ts">
import { computed } from "vue";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import {
  getLocationRulesCount,
  getMappingSecurityIndicatorState,
  isProxyHostMapping,
} from "./model";
import type { SubdomainMappingStatusIndicatorsProps } from "./subdomain-mapping-status-contract";
import SubdomainMappingAccessIndicators from "./SubdomainMappingAccessIndicators.vue";
import SubdomainMappingAvailabilityIndicators from "./SubdomainMappingAvailabilityIndicators.vue";
import SubdomainMappingSecurityIndicators from "./SubdomainMappingSecurityIndicators.vue";

const props = defineProps<SubdomainMappingStatusIndicatorsProps>();
const locationRulesCount = computed(() => getLocationRulesCount(props.mapping));
const securityIndicators = computed(() =>
  getMappingSecurityIndicatorState({
    globalVisibilityEnabled: props.globalVisibilityEnabled,
    globalWafEnabled: props.globalWafEnabled,
    isAuthService: props.isAuthService,
    mapping: props.mapping,
  }),
);
const shouldShowToolbarIndicator = computed(
  () =>
    props.isGatewayPortalEnabled &&
    isProxyHostMapping(props.mapping) &&
    props.mapping.use_auth &&
    !props.mapping.suppress_toolbar &&
    !isWebSocketProxyTargetUrl(props.mapping.target),
);
</script>

<template>
  <div
    class="flex min-w-max flex-nowrap items-center gap-2 text-xs text-muted-foreground"
  >
    <SubdomainMappingAvailabilityIndicators :model="props" />
    <template v-if="availabilityState !== 'disabled'">
      <SubdomainMappingAccessIndicators :model="props" />
      <SubdomainMappingSecurityIndicators
        :location-rules-count="locationRulesCount"
        :model="props"
        :security-indicators="securityIndicators"
        :should-show-toolbar-indicator="shouldShowToolbarIndicator"
      />
    </template>
  </div>
</template>
