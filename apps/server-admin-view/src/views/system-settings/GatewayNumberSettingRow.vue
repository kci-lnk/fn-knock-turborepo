<script setup lang="ts">
import { computed } from "vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";

const props = withDefaults(
  defineProps<{
    disabled?: boolean;
    min?: number;
    modelValue: number;
    step?: number;
    summary?: string;
    title: string;
    unitLabel: string;
    unitWidthClass?: string;
  }>(),
  {
    disabled: false,
    min: 1,
    step: 1,
    summary: "",
    unitWidthClass: "w-16",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: number];
}>();

const localValue = computed({
  get: () => props.modelValue,
  set: (value) => emit("update:modelValue", value),
});
</script>

<template>
  <div
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <Label class="text-base">{{ title }}</Label>
      <div class="text-sm text-muted-foreground">
        <slot name="description" />
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Input
        v-model.number="localValue"
        type="number"
        :min="min"
        :step="step"
        class="w-24 text-center"
        :disabled="disabled"
      />
      <span class="text-sm text-muted-foreground" :class="unitWidthClass">
        {{ unitLabel }}
      </span>
    </div>
    <div
      v-if="summary"
      class="sm:col-span-2 -mt-1 text-xs text-muted-foreground"
    >
      {{ summary }}
    </div>
  </div>
</template>
