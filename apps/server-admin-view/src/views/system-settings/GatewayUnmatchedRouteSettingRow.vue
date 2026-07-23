<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import type { GatewayUnmatchedRouteBehavior } from "@/types";

const props = defineProps<{
  description: string;
  disabled?: boolean;
  errorPageLabel: string;
  modelValue: GatewayUnmatchedRouteBehavior;
  resetConnectionLabel: string;
  title: string;
  warning: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: GatewayUnmatchedRouteBehavior];
}>();

const selectBehavior = (value: GatewayUnmatchedRouteBehavior) => {
  if (!props.disabled) emit("update:modelValue", value);
};
</script>

<template>
  <div
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <Label class="text-base">{{ title }}</Label>
      <div class="text-sm text-muted-foreground">
        {{ description }}
      </div>
      <div
        v-if="modelValue === 'reset_connection'"
        class="text-xs leading-5 text-amber-600 dark:text-amber-300"
      >
        {{ warning }}
      </div>
    </div>
    <div class="inline-flex w-fit rounded-md border bg-background p-1">
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'error_page' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'error_page'"
        @click="selectBehavior('error_page')"
      >
        {{ errorPageLabel }}
      </Button>
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'reset_connection' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'reset_connection'"
        @click="selectBehavior('reset_connection')"
      >
        {{ resetConnectionLabel }}
      </Button>
    </div>
  </div>
</template>
