<script setup lang="ts">
import { GripVertical } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import { Checkbox } from "@/components/ui/checkbox";
import { TableCell, TableRow } from "@/components/ui/table";
import type { HostMapping } from "@/types";
import { getMappingFaviconSrc } from "./model";
import SubdomainMappingRowActions from "./SubdomainMappingRowActions.vue";
import SubdomainMappingStatusIndicators from "./SubdomainMappingStatusIndicators.vue";
import SubdomainMappingTargetCell from "./SubdomainMappingTargetCell.vue";
import SubdomainMappingTitleCell from "./SubdomainMappingTitleCell.vue";
import type {
  SubdomainMappingsCardProps,
  SubdomainMappingsTableActions,
} from "./subdomain-mappings-card-contract";

defineProps<{
  actions: SubdomainMappingsTableActions;
  deepMonitorActive: boolean;
  dragDisabled: boolean;
  mapping: HostMapping;
  model: SubdomainMappingsCardProps;
  selected: boolean;
  selectable: boolean;
  selectionCheckboxClass: string;
  selectionVisibilityClass: string;
  selectionMode: boolean;
  showGroupedView: boolean;
}>();
const emit = defineEmits<{ select: [selected: boolean] }>();
const { t } = useI18n();
</script>

<template>
  <TableRow
    :key="mapping.host"
    :data-host-mapping="mapping.host"
    class="mapping-row"
    :class="[
      'group',
      model.isMappingUnavailable(mapping) ? 'text-muted-foreground' : '',
      deepMonitorActive ? 'bg-primary/[0.04]' : '',
    ]"
  >
    <TableCell class="mapping-sticky-cell mapping-order-cell mapping-icon-cell">
      <div
        class="flex h-7 w-full items-center"
        :class="
          selectionMode ? 'justify-center' : showGroupedView ? 'gap-1 pl-7' : ''
        "
      >
        <Checkbox
          v-if="selectionMode && selectable"
          :class="[
            selectionCheckboxClass,
            selectionVisibilityClass,
            'shrink-0',
          ]"
          :model-value="selected"
          :aria-label="
            t('admin.subdomainProxy.selectMapping', {
              host: model.formatHost(mapping.host),
            })
          "
          @update:model-value="emit('select', $event === true)"
        />
        <button
          v-if="!selectionMode"
          type="button"
          class="mapping-drag-handle inline-flex h-7 items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
          :class="showGroupedView ? 'w-5' : '-ml-1 w-7'"
          :disabled="dragDisabled"
          :aria-label="t('admin.subdomainProxy.dragSortAria')"
        >
          <GripVertical class="h-4 w-4" />
        </button>
      </div>
    </TableCell>
    <TableCell
      class="mapping-sticky-cell mapping-favicon-cell mapping-icon-cell"
    >
      <img
        v-if="getMappingFaviconSrc(mapping) && !model.isFaviconBroken(mapping)"
        :src="getMappingFaviconSrc(mapping)"
        :alt="`${model.getMappingTitleForDisplay(mapping)} favicon`"
        class="h-4 w-4 object-contain transition-opacity"
        :class="{ 'opacity-45': model.isMappingUnavailable(mapping) }"
        @error="model.markFaviconBroken(mapping)"
      />
    </TableCell>
    <SubdomainMappingTitleCell
      :deep-monitor-active="deepMonitorActive"
      :format-host="model.formatHost"
      :get-mapping-title-for-display="model.getMappingTitleForDisplay"
      :handle-protocol-headers-warning-open-change="
        model.handleProtocolHeadersWarningOpenChange
      "
      :is-protocol-headers-warning-open="model.isProtocolHeadersWarningOpen"
      :mapping="mapping"
      :open-protocol-headers-warning="model.openProtocolHeadersWarning"
      :schedule-close-protocol-headers-warning="
        model.scheduleCloseProtocolHeadersWarning
      "
      :should-show-protocol-headers-warning="
        model.shouldShowProtocolHeadersWarning
      "
      :toggle-protocol-headers-warning="model.toggleProtocolHeadersWarning"
      @edit="actions.edit"
    />
    <TableCell class="break-all font-medium">
      <button
        type="button"
        class="inline-flex max-w-full items-start rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
        :class="{
          'text-muted-foreground hover:text-foreground':
            model.isMappingUnavailable(mapping),
        }"
        :title="
          t('admin.subdomainProxy.copyHostTitle', {
            host: model.formatHost(mapping.host),
          })
        "
        :aria-label="
          t('admin.subdomainProxy.copyHostAria', {
            host: model.formatHost(mapping.host),
          })
        "
        @click="actions.copyHost(mapping)"
      >
        <span class="break-all">{{ model.formatHost(mapping.host) }}</span>
      </button>
    </TableCell>
    <SubdomainMappingTargetCell
      :mapping="mapping"
      :unavailable="model.isMappingUnavailable(mapping)"
    />
    <TableCell class="w-[7rem] min-w-[7rem] max-w-[7rem]">
      <HostTrafficActivity
        :host="mapping.host"
        :title="model.getMappingTitleForDisplay(mapping)"
        :sample="model.getHostTrafficSample(mapping.host)"
        :timestamp="model.trafficTimestamp ?? null"
      />
    </TableCell>
    <TableCell class="w-[8rem] min-w-[8rem]">
      <SubdomainMappingStatusIndicators
        :mapping="mapping"
        :availability-state="model.getAvailabilityState(mapping)"
        :availability-window="model.formatAvailabilityWindow(mapping)"
        :format-host="model.formatHost"
        :global-visibility-enabled="model.globalVisibilityEnabled"
        :global-waf-enabled="model.globalWafEnabled"
        :is-auth-service="model.isAuthServiceTarget(mapping.target)"
        :is-gateway-portal-enabled="model.isGatewayPortalEnabled"
        :is-default-domain-available="model.isDefaultDomainAvailable"
        :is-mapping-status-tooltip-open="model.isMappingStatusTooltipOpen"
        :handle-mapping-status-tooltip-open-change="
          model.handleMappingStatusTooltipOpenChange
        "
        :handle-mapping-status-tooltip-trigger-click="
          model.handleMappingStatusTooltipTriggerClick
        "
      />
    </TableCell>
    <SubdomainMappingRowActions
      :can-use-deep-monitor="model.canUseDeepMonitor"
      :deep-monitor-active="deepMonitorActive"
      :groups="model.groups"
      :is-auth-service-target="model.isAuthServiceTarget"
      :is-default-domain-available="model.isDefaultDomainAvailable"
      :is-saving-mappings="model.isSavingMappings"
      :mapping="mapping"
      @clear-default="actions.clearDefault"
      @delete="actions.deleteMapping"
      @edit="actions.edit"
      @move="
        (movedMapping, groupId) =>
          actions.moveMappings([movedMapping.host], groupId)
      "
      @open-advanced-auth="actions.openAdvancedAuth"
      @open-availability="actions.openAvailability"
      @open-deep-monitor="actions.openDeepMonitor"
      @open-gateway-locations="actions.openGatewayLocations"
      @set-default="actions.setDefault"
      @toggle-enabled="actions.toggleEnabled"
    />
  </TableRow>
</template>
