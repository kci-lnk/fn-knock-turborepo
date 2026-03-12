<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from 'vue';
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
let timer: number | null = null;

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

onUnmounted(() => {
  stopTimer();
});
</script>

<template>
  <span v-if="!showTooltip">{{ displayText }}</span>
  <TooltipProvider v-else>
    <Tooltip>
      <TooltipTrigger as-child>
        <span class="cursor-help">{{ displayText }}</span>
      </TooltipTrigger>
      <TooltipContent>
        <p>{{ fullText }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
