<script setup lang="ts">
import { Button } from '@/components/ui/button';
import { Popover, PopoverContent, PopoverTrigger } from '@/components/ui/popover';
import { Loader2 } from 'lucide-vue-next';
import type { ButtonVariants } from '@/components/ui/button';

const props = withDefaults(
  defineProps<{
    title: string;
    description: string;
    confirmText?: string;
    cancelText?: string;
    confirmVariant?: ButtonVariants['variant'];
    loading?: boolean;
    disabled?: boolean;
    contentClass?: string;
    closeOnConfirm?: boolean;
    onConfirm: () => void | Promise<void>;
  }>(),
  {
    confirmText: '确认删除',
    cancelText: '取消',
    confirmVariant: 'destructive',
    loading: false,
    disabled: false,
    contentClass: 'w-72 text-left',
    closeOnConfirm: true,
  },
);

const handleConfirm = async (close: () => void) => {
  await props.onConfirm();
  if (props.closeOnConfirm) {
    close();
  }
};
</script>

<template>
  <Popover v-slot="{ close }">
    <PopoverTrigger as-child>
      <slot name="trigger" />
    </PopoverTrigger>
    <PopoverContent :class="props.contentClass">
      <div class="grid gap-3">
        <p class="text-sm font-medium">{{ props.title }}</p>
        <p class="text-xs text-muted-foreground">{{ props.description }}</p>
        <div class="flex justify-end gap-2">
          <Button variant="outline" size="sm" @click="close" :disabled="props.loading">
            {{ props.cancelText }}
          </Button>
          <Button
            :variant="props.confirmVariant"
            size="sm"
            :disabled="props.disabled || props.loading"
            @click="handleConfirm(close)"
          >
            <Loader2 v-if="props.loading" class="mr-2 h-3 w-3 animate-spin" />
            {{ props.confirmText }}
          </Button>
        </div>
      </div>
    </PopoverContent>
  </Popover>
</template>
