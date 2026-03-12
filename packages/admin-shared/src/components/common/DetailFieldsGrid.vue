<script setup lang="ts">
import { computed } from 'vue';

interface DetailFieldItem {
  key: string;
  label: string;
  value: string | number | boolean;
}

const props = withDefaults(
  defineProps<{
    items: DetailFieldItem[];
    layout?: 'compact' | 'card';
    cardGridClass?: string;
  }>(),
  {
    layout: 'compact',
    cardGridClass: 'md:grid-cols-2',
  },
);

const visibleItems = computed(() => props.items.filter((item) => item.value !== undefined));
</script>

<template>
  <div
    v-if="props.layout === 'card'"
    class="grid gap-3"
    :class="props.cardGridClass"
  >
    <div
      v-for="item in visibleItems"
      :key="item.key"
      class="border rounded-lg p-4 space-y-2"
    >
      <div class="text-sm text-muted-foreground">{{ item.label }}</div>
      <div class="text-base break-all">{{ item.value }}</div>
    </div>
  </div>
  <div v-else class="grid gap-4">
    <div
      v-for="item in visibleItems"
      :key="item.key"
      class="grid grid-cols-4 items-start gap-4"
    >
      <span class="text-right font-medium text-muted-foreground pt-1">{{ item.label }}</span>
      <div class="col-span-3 text-sm bg-muted/50 p-2 rounded-md break-all">
        {{ item.value }}
      </div>
    </div>
  </div>
</template>
