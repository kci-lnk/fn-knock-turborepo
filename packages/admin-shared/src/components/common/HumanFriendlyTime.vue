<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from 'vue';
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip';
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe';
import { formatHumanFriendlyTime, resolveDateValue } from '@admin-shared/utils/formatHumanFriendlyTime';

const props = withDefaults(defineProps<{
  value: string | number | Date | null | undefined;
  locale?: string;
  emptyText?: string;
  keepInvalidRawText?: boolean;
  absoluteFormatOptions?: Intl.DateTimeFormatOptions;
  refreshIntervalMs?: number;
}>(), {
  locale: 'zh-CN',
  emptyText: '-',
  keepInvalidRawText: true,
  refreshIntervalMs: 60_000,
});

const now = ref(Date.now());
const open = ref(false);
const isTouchInteraction = ref(false);
let timer: number | null = null;
let interactionMediaQuery: MediaQueryList | null = null;

const updateInteractionMode = () => {
  if (typeof window === 'undefined') {
    return;
  }

  isTouchInteraction.value = window.matchMedia('(hover: none), (pointer: coarse)').matches;
};

const stopTimer = () => {
  if (timer !== null) {
    window.clearInterval(timer);
    timer = null;
  }
};

const startTimer = () => {
  stopTimer();
  timer = window.setInterval(() => {
    now.value = Date.now();
  }, props.refreshIntervalMs);
};

const resolvedDate = computed(() => resolveDateValue(props.value));
const fullText = computed(() =>
  formatDateTimeSafe(props.value, {
    locale: props.locale,
    emptyText: props.emptyText,
    keepInvalidRawText: props.keepInvalidRawText,
    formatOptions: props.absoluteFormatOptions,
  }),
);
const displayText = computed(() =>
  formatHumanFriendlyTime(props.value, {
    locale: props.locale,
    emptyText: props.emptyText,
    keepInvalidRawText: props.keepInvalidRawText,
    now: now.value,
  }),
);
const showTooltip = computed(() => Boolean(resolvedDate.value) && fullText.value !== displayText.value);

const handleOpenChange = (nextOpen: boolean) => {
  open.value = nextOpen;
};

const handleTriggerClick = () => {
  if (!showTooltip.value || !isTouchInteraction.value) {
    return;
  }

  open.value = !open.value;
};

watch(
  [resolvedDate, () => props.refreshIntervalMs],
  ([date]) => {
    now.value = Date.now();
    if (!date) {
      stopTimer();
      return;
    }
    startTimer();
  },
  { immediate: true },
);

watch(showTooltip, (visible) => {
  if (!visible) {
    open.value = false;
  }
});

onMounted(() => {
  if (typeof window === 'undefined') {
    return;
  }

  interactionMediaQuery = window.matchMedia('(hover: none), (pointer: coarse)');
  updateInteractionMode();

  if (typeof interactionMediaQuery.addEventListener === 'function') {
    interactionMediaQuery.addEventListener('change', updateInteractionMode);
    return;
  }

  interactionMediaQuery.addListener(updateInteractionMode);
});

onUnmounted(() => {
  stopTimer();

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
  <span v-if="!showTooltip">{{ displayText }}</span>
  <TooltipProvider v-else>
    <Tooltip :open="open" @update:open="handleOpenChange">
      <TooltipTrigger as-child>
        <span
          class="cursor-help"
          tabindex="0"
          @click="handleTriggerClick"
        >
          {{ displayText }}
        </span>
      </TooltipTrigger>
      <TooltipContent>
        <p>{{ fullText }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
