<script setup lang="ts">
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from 'vue';
import type { HTMLAttributes } from 'vue';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { cn } from '@/lib/utils';

const props = withDefaults(defineProps<{
  text: string | number | null | undefined;
  emptyText?: string;
  as?: string;
  class?: HTMLAttributes['class'];
  tooltipClass?: HTMLAttributes['class'];
  tooltipSide?: 'top' | 'right' | 'bottom' | 'left';
  tooltipAlign?: 'start' | 'center' | 'end';
}>(), {
  emptyText: '-',
  as: 'span',
  tooltipSide: 'top',
  tooltipAlign: 'center',
});

const open = ref(false);
const isOverflowing = ref(false);
const isTouchInteraction = ref(false);
const textRef = ref<HTMLElement | null>(null);

let resizeObserver: ResizeObserver | null = null;
let interactionMediaQuery: MediaQueryList | null = null;

const resolvedText = computed(() => {
  const value = props.text;
  if (value === null || value === undefined || value === '') {
    return props.emptyText;
  }
  return String(value);
});

const showTooltip = computed(() => isOverflowing.value && resolvedText.value !== props.emptyText);

const updateInteractionMode = () => {
  if (typeof window === 'undefined') {
    return;
  }

  isTouchInteraction.value = window.matchMedia('(hover: none), (pointer: coarse)').matches;
};

const measureOverflow = async () => {
  await nextTick();

  const element = textRef.value;
  if (!element) {
    isOverflowing.value = false;
    return;
  }

  isOverflowing.value = element.scrollWidth - element.clientWidth > 1;
};

const handleOpenChange = (nextOpen: boolean) => {
  open.value = nextOpen;
};

const handleTriggerClick = () => {
  if (!showTooltip.value || !isTouchInteraction.value) {
    return;
  }

  open.value = !open.value;
};

const reconnectResizeObserver = (element: HTMLElement | null) => {
  resizeObserver?.disconnect();

  if (!element || typeof ResizeObserver === 'undefined') {
    return;
  }

  resizeObserver = new ResizeObserver(() => {
    void measureOverflow();
  });
  resizeObserver.observe(element);
};

watch(() => resolvedText.value, () => {
  void measureOverflow();
}, { immediate: true });

watch(textRef, (element) => {
  reconnectResizeObserver(element);
  void measureOverflow();
});

watch(showTooltip, (visible) => {
  if (!visible) {
    open.value = false;
  }
});

onMounted(() => {
  updateInteractionMode();
  void measureOverflow();

  if (typeof window !== 'undefined') {
    interactionMediaQuery = window.matchMedia('(hover: none), (pointer: coarse)');

    if (typeof interactionMediaQuery.addEventListener === 'function') {
      interactionMediaQuery.addEventListener('change', updateInteractionMode);
    }
    else {
      interactionMediaQuery.addListener(updateInteractionMode);
    }
  }

  reconnectResizeObserver(textRef.value);
});

onUnmounted(() => {
  resizeObserver?.disconnect();

  if (!interactionMediaQuery) {
    return;
  }

  if (typeof interactionMediaQuery.removeEventListener === 'function') {
    interactionMediaQuery.removeEventListener('change', updateInteractionMode);
    return;
  }

  interactionMediaQuery.removeListener(updateInteractionMode);
});
</script>

<template>
  <component
    :is="as"
    v-if="!showTooltip"
    ref="textRef"
    :class="cn('block min-w-0 max-w-full truncate', props.class)"
  >
    {{ resolvedText }}
  </component>

  <TooltipProvider v-else>
    <Tooltip
      :open="open"
      @update:open="handleOpenChange"
    >
      <TooltipTrigger as-child>
        <component
          :is="as"
          ref="textRef"
          :class="cn('block min-w-0 max-w-full truncate cursor-help', props.class)"
          tabindex="0"
          @click="handleTriggerClick"
        >
          {{ resolvedText }}
        </component>
      </TooltipTrigger>
      <TooltipContent
        :side="tooltipSide"
        :align="tooltipAlign"
        :class="cn('max-w-[min(32rem,calc(100vw-2rem))] break-all', props.tooltipClass)"
      >
        <p>{{ resolvedText }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
