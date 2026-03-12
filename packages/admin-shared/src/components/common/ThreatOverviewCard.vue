<script setup lang="ts">
import type { Component } from 'vue';
import { computed } from 'vue';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Skeleton } from '@/components/ui/skeleton';
import type { ThreatRange } from '@admin-shared/composables/useThreatOverview';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';

const props = withDefaults(defineProps<{
  title: string;
  description: string;
  rangeKey?: string;
  ranges?: ReadonlyArray<ThreatRange>;
  isLoading: boolean;
  titleRangeText: string;
  primaryLabel: string;
  primaryValue: string;
  primaryHint: string;
  secondaryLabel: string;
  secondaryValue: string;
  secondaryHint: string;
  icon: Component;
  primaryIcon?: Component;
  secondaryIcon?: Component;
  showRangeTabs?: boolean;
  chartHeightClass?: string;
}>(), {
  rangeKey: '',
  ranges: () => [],
  primaryIcon: undefined,
  secondaryIcon: undefined,
  showRangeTabs: true,
  chartHeightClass: 'h-[240px]',
});

const emit = defineEmits<{
  'update:rangeKey': [value: string];
}>();

const showLoadingSkeleton = useDelayedLoading(() => props.isLoading);

const currentRangeKey = computed({
  get: () => props.rangeKey,
  set: (value: string) => {
    emit('update:rangeKey', value);
  },
});
</script>

<template>
  <Card class="overflow-hidden">
    <CardHeader class="pb-3">
      <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div class="flex items-center gap-3">
          <div>
            <CardTitle class="text-base">{{ props.title }}</CardTitle>
            <CardDescription>{{ props.description }}（{{ props.titleRangeText }}）</CardDescription>
          </div>
        </div>
        <div v-if="$slots['header-right']" class="flex flex-wrap items-center gap-2">
          <slot name="header-right" />
        </div>
        <Tabs v-else-if="props.showRangeTabs" v-model="currentRangeKey" class="w-full sm:w-auto">
          <TabsList class="w-full sm:w-auto">
            <TabsTrigger v-for="range in props.ranges" :key="range.key" :value="range.key">
              {{ range.key }}
            </TabsTrigger>
          </TabsList>
        </Tabs>
      </div>
    </CardHeader>
    <CardContent class="pt-0">
      <div v-if="props.isLoading && showLoadingSkeleton" class="space-y-3">
        <Skeleton class="h-6 w-56" />
        <Skeleton class="w-full" :class="props.chartHeightClass" />
      </div>
      <div v-else-if="!props.isLoading" class="grid gap-4">
        <div class="grid gap-3 sm:grid-cols-2">
          <div class="rounded-lg border bg-muted/20 px-3 py-2.5">
            <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
              <component :is="props.primaryIcon || props.icon" class="h-3.5 w-3.5" />
              {{ props.primaryLabel }}
            </div>
            <div class="mt-2 text-2xl font-semibold tracking-tight">{{ props.primaryValue }}</div>
            <div class="text-[11px] text-muted-foreground mt-1">{{ props.primaryHint }}</div>
          </div>
          <div class="rounded-lg border bg-muted/20 px-3 py-2.5">
            <div class="flex items-center gap-1.5 text-xs text-muted-foreground">
              <component :is="props.secondaryIcon || props.icon" class="h-3.5 w-3.5" />
              {{ props.secondaryLabel }}
            </div>
            <div class="mt-2 text-2xl font-semibold tracking-tight">{{ props.secondaryValue }}</div>
            <div class="text-[11px] text-muted-foreground mt-1">{{ props.secondaryHint }}</div>
          </div>
        </div>
        <div class="w-full" :class="props.chartHeightClass">
          <slot name="chart" />
        </div>
      </div>
      <div v-else class="h-[264px]" aria-hidden="true" ></div>
    </CardContent>
  </Card>
</template>
