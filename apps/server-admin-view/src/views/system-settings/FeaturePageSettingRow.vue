<script setup lang="ts">
import { ChevronRight } from "lucide-vue-next";

defineProps<{
  title: string;
  description: string;
  available: boolean;
  disabledReason: string;
}>();

defineEmits<{ open: [] }>();
</script>

<template>
  <button
    type="button"
    class="flex w-full items-center justify-between p-6 text-left transition-colors"
    :class="
      available
        ? 'bg-muted/5 hover:bg-muted/15'
        : 'cursor-not-allowed bg-muted/5'
    "
    :disabled="!available"
    @click="$emit('open')"
  >
    <div class="space-y-1 pr-6">
      <div
        class="text-base font-medium"
        :class="available ? 'text-foreground' : 'text-zinc-500'"
      >
        {{ title }}
      </div>
      <div
        class="text-sm"
        :class="available ? 'text-muted-foreground' : 'text-zinc-500'"
      >
        {{ description }}
      </div>
      <div v-if="!available" class="text-xs leading-5 text-zinc-500">
        {{ disabledReason }}
      </div>
    </div>
    <ChevronRight
      class="h-5 w-5 shrink-0"
      :class="available ? 'text-muted-foreground' : 'text-zinc-400'"
    />
  </button>
</template>
