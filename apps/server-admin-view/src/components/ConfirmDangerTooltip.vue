<script setup lang="ts">
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
defineProps<{
  label: string;
  title: string;
  description: string;
  loading?: boolean;
  disabled?: boolean;
  onConfirm: () => void | Promise<void>;
}>();
</script>

<template>
  <TooltipProvider>
    <Tooltip>
      <ConfirmDangerPopover
        :title="title"
        :description="description"
        :loading="loading"
        :disabled="disabled"
        :on-confirm="onConfirm"
      >
        <template #trigger>
          <TooltipTrigger as-child><slot name="trigger" /></TooltipTrigger>
        </template>
      </ConfirmDangerPopover>
      <TooltipContent side="top" align="center">{{ label }}</TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
