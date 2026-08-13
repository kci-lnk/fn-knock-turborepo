<script setup lang="ts">
import { Button } from "@/components/ui/button";

defineProps<{
  title: string;
  description: string;
  modelValue: string;
  options: Array<{ label: string; value: string }>;
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
</script>

<template>
  <section
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <div class="text-base font-medium">{{ title }}</div>
      <div class="text-sm text-muted-foreground">{{ description }}</div>
    </div>
    <div
      role="group"
      :aria-label="title"
      class="inline-flex w-fit rounded-md border bg-background p-1"
    >
      <Button
        v-for="option in options"
        :key="option.value"
        type="button"
        size="sm"
        variant="ghost"
        :aria-pressed="modelValue === option.value"
        :class="[
          'h-8 px-3',
          modelValue === option.value
            ? 'bg-foreground text-background hover:bg-foreground/90 hover:text-background'
            : '',
        ]"
        :disabled="disabled"
        @click="emit('update:modelValue', option.value)"
      >
        {{ option.label }}
      </Button>
    </div>
  </section>
</template>
