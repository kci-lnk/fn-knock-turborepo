<script setup lang="ts">
import { useSlots } from 'vue';
import { Card, CardContent, CardDescription, CardFooter, CardHeader, CardTitle } from '@/components/ui/card';
import { Skeleton } from '@/components/ui/skeleton';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';

const props = defineProps<{
  title: string;
  description: string;
  isInitializing: boolean;
}>();

const slots = useSlots();
const showInitializingSkeleton = useDelayedLoading(() => props.isInitializing);
</script>

<template>
  <Card class="w-full">
    <CardHeader>
      <CardTitle>{{ props.title }}</CardTitle>
      <CardDescription>{{ props.description }}</CardDescription>
    </CardHeader>

    <CardContent v-if="props.isInitializing && showInitializingSkeleton" class="grid gap-6">
      <slot name="initial" />
    </CardContent>
    <CardContent v-else-if="!props.isInitializing" class="grid gap-6">
      <slot />
    </CardContent>
    <CardContent v-else class="min-h-[160px]" aria-hidden="true" ></CardContent>

    <CardFooter
      v-if="props.isInitializing && slots.footer && showInitializingSkeleton"
      class="flex justify-end gap-3 border-t pt-6"
    >
      <slot name="initial-footer">
        <Skeleton class="h-10 w-28" />
      </slot>
    </CardFooter>
    <CardFooter
      v-else-if="!props.isInitializing && slots.footer"
      class="flex justify-end gap-3 border-t pt-6"
    >
      <slot name="footer" />
    </CardFooter>
  </Card>
</template>
