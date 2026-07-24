<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Tooltip,
  TooltipContent,
  TooltipProvider,
  TooltipTrigger,
} from "@/components/ui/tooltip";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import {
  formatHumanFriendlyTime,
  resolveDateValue,
} from "@admin-shared/utils/formatHumanFriendlyTime";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";

const props = withDefaults(
  defineProps<{
    value: string | number | Date | null | undefined;
    locale?: string;
    emptyText?: string;
    keepInvalidRawText?: boolean;
    absoluteFormatOptions?: Intl.DateTimeFormatOptions;
    refreshIntervalMs?: number;
    tooltipLines?: string[];
  }>(),
  {
    emptyText: "-",
    keepInvalidRawText: true,
    refreshIntervalMs: 60_000,
  },
);

const { locale: globalLocale } = useI18n({ useScope: "global" });
const now = ref(Date.now());
const open = ref(false);
const isTouchInteraction = useMediaQueryMatch(
  "(hover: none), (pointer: coarse)",
);
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
const effectiveLocale = computed(() =>
  String(props.locale || globalLocale.value || "zh-CN"),
);
const fullText = computed(() =>
  formatDateTimeSafe(props.value, {
    locale: effectiveLocale.value,
    emptyText: props.emptyText,
    keepInvalidRawText: props.keepInvalidRawText,
    formatOptions: props.absoluteFormatOptions,
  }),
);
const displayText = computed(() =>
  formatHumanFriendlyTime(props.value, {
    locale: effectiveLocale.value,
    emptyText: props.emptyText,
    keepInvalidRawText: props.keepInvalidRawText,
    now: now.value,
  }),
);
const customTooltipLines = computed(() =>
  (props.tooltipLines || []).map((line) => line?.trim()).filter(Boolean),
);
const tooltipContentLines = computed(() =>
  customTooltipLines.value.length > 0
    ? customTooltipLines.value
    : [fullText.value],
);
const showTooltip = computed(
  () =>
    customTooltipLines.value.length > 0 ||
    (Boolean(resolvedDate.value) && fullText.value !== displayText.value),
);

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

onUnmounted(() => {
  stopTimer();
});
</script>

<template>
  <span v-if="!showTooltip">{{ displayText }}</span>
  <TooltipProvider v-else>
    <Tooltip :open="open" @update:open="handleOpenChange">
      <TooltipTrigger as-child>
        <button
          type="button"
          class="cursor-help border-0 bg-transparent p-0 font-inherit text-inherit"
          @click="handleTriggerClick"
        >
          {{ displayText }}
        </button>
      </TooltipTrigger>
      <TooltipContent>
        <p v-for="line in tooltipContentLines" :key="line">{{ line }}</p>
      </TooltipContent>
    </Tooltip>
  </TooltipProvider>
</template>
