<script setup lang="ts">
import { computed } from 'vue';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { Loader2 } from 'lucide-vue-next';

const props = withDefaults(
  defineProps<{
    open: boolean;
    title: string;
    description?: string;
    maxWidthClass?: string;
    loading?: boolean;
    closeText?: string;
    closeVariant?: 'default' | 'outline' | 'secondary' | 'ghost' | 'destructive' | 'link';
    showFooter?: boolean;
  }>(),
  {
    description: '',
    maxWidthClass: 'sm:max-w-[700px]',
    loading: false,
    closeText: '关闭',
    closeVariant: 'outline',
    showFooter: true,
  },
);

const emit = defineEmits<{
  'update:open': [value: boolean];
}>();

const modelOpen = computed({
  get: () => props.open,
  set: (value: boolean) => emit('update:open', value),
});

const close = () => {
  modelOpen.value = false;
};
</script>

<template>
  <Dialog v-model:open="modelOpen">
    <DialogContent :class="props.maxWidthClass">
      <DialogHeader>
        <DialogTitle>{{ props.title }}</DialogTitle>
        <DialogDescription v-if="props.description">{{ props.description }}</DialogDescription>
      </DialogHeader>

      <div v-if="props.loading" class="py-10 text-center text-muted-foreground">
        <Loader2 class="h-6 w-6 animate-spin mx-auto" />
      </div>
      <slot v-else />

      <DialogFooter v-if="props.showFooter">
        <Button :variant="props.closeVariant" @click="close">{{ props.closeText }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
