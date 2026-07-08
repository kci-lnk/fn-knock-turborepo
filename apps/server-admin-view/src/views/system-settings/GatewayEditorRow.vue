<script setup lang="ts">
import { computed } from "vue";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";

const props = withDefaults(
  defineProps<{
    actionLabel: string;
    description: string;
    disabled?: boolean;
    disabledReason?: string;
    title: string;
  }>(),
  {
    disabled: false,
    disabledReason: "",
  },
);

const emit = defineEmits<{
  action: [];
}>();

const mutedClass = computed(() =>
  props.disabled ? "text-zinc-500" : "text-muted-foreground",
);
</script>

<template>
  <div class="grid gap-4 p-6 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
    <div class="space-y-3">
      <div class="flex flex-wrap items-center gap-2">
        <Label class="text-base" :class="disabled ? 'text-zinc-500' : ''">
          {{ title }}
        </Label>
        <slot name="badges" />
      </div>
      <div class="text-sm leading-6" :class="mutedClass">
        {{ description }}
      </div>
      <div
        v-if="disabled && disabledReason"
        class="text-xs leading-5 text-zinc-500"
      >
        {{ disabledReason }}
      </div>
    </div>
    <div class="flex justify-start lg:justify-end">
      <Button variant="outline" :disabled="disabled" @click="emit('action')">
        {{ actionLabel }}
      </Button>
    </div>
  </div>
</template>
