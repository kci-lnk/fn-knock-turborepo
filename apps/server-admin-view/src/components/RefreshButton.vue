<script setup lang="ts">
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { HTMLAttributes } from 'vue';
import type { ButtonVariants } from '@/components/ui/button';
import { Button } from '@/components/ui/button';
import { RefreshCw } from 'lucide-vue-next';

interface Props {
  loading?: boolean;
  disabled?: boolean;
  label?: string;
  size?: ButtonVariants['size'];
  iconOnly?: boolean;
  class?: HTMLAttributes['class'];
}

const props = withDefaults(defineProps<Props>(), {
  loading: false,
  disabled: false,
  label: '刷新',
  size: 'sm',
  iconOnly: false,
});

defineEmits<{
  click: [];
}>();

const MIN_SPIN_DURATION_MS = 500;

const animationActive = ref(props.loading);
const spinStartedAt = ref<number | null>(props.loading ? Date.now() : null);
let stopTimer: ReturnType<typeof setTimeout> | null = null;

const clearStopTimer = () => {
  if (stopTimer) {
    clearTimeout(stopTimer);
    stopTimer = null;
  }
};

watch(
  () => props.loading,
  (loading) => {
    clearStopTimer();

    if (loading) {
      animationActive.value = true;
      spinStartedAt.value = Date.now();
      return;
    }

    if (!animationActive.value) {
      return;
    }

    const elapsed = spinStartedAt.value ? Date.now() - spinStartedAt.value : MIN_SPIN_DURATION_MS;
    const remaining = Math.max(MIN_SPIN_DURATION_MS - elapsed, 0);

    stopTimer = setTimeout(() => {
      animationActive.value = false;
      spinStartedAt.value = null;
      stopTimer = null;
    }, remaining);
  },
);

onBeforeUnmount(() => {
  clearStopTimer();
});

const iconClass = computed(() => ({
  'mr-1.5': !props.iconOnly,
  'animate-spin': animationActive.value,
}));
</script>

<template>
  <Button
    variant="outline"
    :size="props.size"
    :disabled="props.disabled"
    :class="props.class"
    :aria-label="props.label"
    @click="$emit('click')"
  >
    <RefreshCw
      class="h-4 w-4"
      :class="iconClass"
    />
    <span v-if="!props.iconOnly">{{ props.label }}</span>
  </Button>
</template>
