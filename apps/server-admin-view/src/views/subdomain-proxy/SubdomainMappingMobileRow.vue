<script setup lang="ts">
import { Globe2, GripVertical } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import HostTrafficActivity from "@/components/HostTrafficActivity.vue";
import { Checkbox } from "@/components/ui/checkbox";
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
  selectionMode: boolean;
}>();

const emit = defineEmits<{ select: [selected: boolean] }>();
const { t } = useI18n();
</script>

<template>
  <article
    :key="mapping.host"
    :data-host-mapping="mapping.host"
    :aria-label="`${model.getMappingTitleForDisplay(mapping)} · ${model.formatHost(mapping.host)}`"
    class="mapping-mobile-row border-b px-3 py-3"
    :class="[
      model.isMappingUnavailable(mapping) ? 'text-muted-foreground' : '',
      deepMonitorActive ? 'bg-primary/[0.04]' : '',
    ]"
  >
    <div class="flex items-start gap-2.5">
      <Checkbox
        v-if="selectionMode && selectable"
        :class="[selectionCheckboxClass, 'mt-1 shrink-0']"
        :model-value="selected"
        :aria-label="
          t('admin.subdomainProxy.selectMapping', {
            host: model.formatHost(mapping.host),
          })
        "
        @update:model-value="emit('select', $event === true)"
      />
      <span
        v-else-if="selectionMode"
        class="mt-1 size-[18px] shrink-0"
        aria-hidden="true"
      ></span>
      <button
        v-else
        type="button"
        class="mapping-drag-handle mt-0.5 inline-flex h-8 w-7 shrink-0 touch-none items-center justify-center rounded-md text-muted-foreground transition hover:bg-muted hover:text-foreground disabled:pointer-events-none disabled:opacity-40"
        :disabled="dragDisabled"
        :aria-label="`${t('admin.subdomainProxy.dragSortAria')}: ${model.formatHost(mapping.host)}`"
      >
        <GripVertical class="h-4 w-4" />
      </button>

      <div
        class="flex h-8 w-8 shrink-0 items-center justify-center rounded-md bg-muted/45"
      >
        <img
          v-if="
            getMappingFaviconSrc(mapping) && !model.isFaviconBroken(mapping)
          "
          :src="getMappingFaviconSrc(mapping)"
          :alt="`${model.getMappingTitleForDisplay(mapping)} favicon`"
          class="h-5 w-5 object-contain transition-opacity"
          :class="{ 'opacity-45': model.isMappingUnavailable(mapping) }"
          @error="model.markFaviconBroken(mapping)"
        />
        <Globe2
          v-else
          class="h-4 w-4 text-muted-foreground/70"
          aria-hidden="true"
        />
      </div>

      <div class="min-w-0 flex-1 pt-0.5">
        <SubdomainMappingTitleCell
          :as-cell="false"
          compact
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

        <button
          type="button"
          class="mt-1 block max-w-full truncate rounded-sm text-left text-xs font-medium text-foreground/85 transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2"
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
          {{ model.formatHost(mapping.host) }}
        </button>

        <SubdomainMappingTargetCell
          :as-cell="false"
          compact
          class="mt-1"
          :mapping="mapping"
          :unavailable="model.isMappingUnavailable(mapping)"
        />
      </div>

      <SubdomainMappingRowActions
        :as-cell="false"
        compact
        :can-use-deep-monitor="model.canUseDeepMonitor"
        :deep-monitor-active="deepMonitorActive"
        :groups="model.groups"
        :is-auth-service-target="model.isAuthServiceTarget"
        :is-default-domain-available="model.isDefaultDomainAvailable"
        :is-saving-mappings="model.isSavingMappings"
        :mapping="mapping"
        :trigger-aria-label="`${t('common.moreActions')}: ${model.formatHost(mapping.host)}`"
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
    </div>

    <div
      class="mt-2.5 flex min-w-0 items-center justify-between gap-2 border-t border-border/60 pt-2"
    >
      <div
        class="flex min-h-7 min-w-0 items-center rounded-md bg-muted/35 px-1.5 py-1"
      >
        <span class="sr-only">
          {{ t("admin.subdomainProxy.columns.status") }}
        </span>
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
      </div>
      <div class="min-w-0 shrink-0 text-right">
        <span class="sr-only">
          {{ t("admin.subdomainProxy.columns.traffic") }}
        </span>
        <HostTrafficActivity
          :host="mapping.host"
          :title="model.getMappingTitleForDisplay(mapping)"
          :sample="model.getHostTrafficSample(mapping.host)"
          :timestamp="model.trafficTimestamp ?? null"
        />
      </div>
    </div>
  </article>
</template>
