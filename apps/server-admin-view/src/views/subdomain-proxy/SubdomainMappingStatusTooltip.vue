<script setup lang="ts">
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import type { SubdomainMappingStatusIndicatorsProps } from "./subdomain-mapping-status-contract";
import type { MappingStatusTooltip } from "./useSubdomainTouchTooltips";

const props = defineProps<{
  model: SubdomainMappingStatusIndicatorsProps;
  tooltip: MappingStatusTooltip;
}>();

const handleOpenChange = (open: boolean) => {
  props.model.handleMappingStatusTooltipOpenChange(
    props.model.mapping.host,
    props.tooltip,
    open,
  );
};

const handleTriggerClick = () => {
  props.model.handleMappingStatusTooltipTriggerClick(
    props.model.mapping.host,
    props.tooltip,
  );
};
</script>

<template>
  <TooltipProvider>
    <Tooltip
      :open="model.isMappingStatusTooltipOpen(model.mapping.host, tooltip)"
      @update:open="handleOpenChange"
    >
      <TooltipTrigger as-child>
        <slot name="trigger" :handle-click="handleTriggerClick" />
      </TooltipTrigger>
      <TooltipContent side="top" align="center">
        <slot />
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
