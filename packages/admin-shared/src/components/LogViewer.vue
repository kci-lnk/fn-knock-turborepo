<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";

export interface LogViewerProps {
  /** Log lines, as strings or arbitrary objects. */
  logs?: unknown[];
  /** Optional title. */
  title?: string;
  /** Whether to show newest first. */
  reversed?: boolean;
  /** Optional empty-state text. */
  emptyText?: string;
  /** Container height class. */
  heightClass?: string;
  /** Whether to wrap log lines. */
  wrap?: boolean;
  /** Whether to show the header. */
  showHeader?: boolean;
  /** Theme: dark terminal style or light panel style. */
  theme?: "dark" | "light";
}

const props = withDefaults(defineProps<LogViewerProps>(), {
  logs: () => [],
  reversed: false,
  heightClass: "h-72",
  wrap: false,
  showHeader: true,
  theme: "light",
});

const { t } = useI18n();

const displayLogs = computed(() =>
  props.reversed ? [...props.logs].reverse() : props.logs,
);

const isDark = computed(() => props.theme === "dark");
const titleText = computed(() => props.title ?? t("shared.logViewer.title"));
const emptyTextLabel = computed(
  () => props.emptyText ?? t("shared.logViewer.emptyText"),
);
</script>

<template>
  <div
    class="overflow-hidden rounded-lg border"
    :class="isDark ? 'bg-black/90' : 'bg-background'"
  >
    <div
      v-if="showHeader"
      class="flex items-center justify-between gap-2 border-b px-3 py-2"
      :class="isDark ? 'border-white/10' : 'border-border'"
    >
      <div
        class="text-xs font-medium"
        :class="isDark ? 'text-white/80' : 'text-foreground'"
      >
        {{ titleText }}
      </div>
      <div
        class="text-xs"
        :class="isDark ? 'text-white/40' : 'text-muted-foreground'"
      >
        {{ t("shared.logViewer.lineCount", { count: logs.length }) }}
      </div>
    </div>
    <div
      :class="[
        heightClass,
        'overflow-auto p-3 font-mono text-xs leading-5',
        isDark ? 'text-green-200' : 'text-foreground',
      ]"
    >
      <div
        v-if="logs.length === 0"
        :class="isDark ? 'text-white/50' : 'text-muted-foreground'"
      >
        {{ emptyTextLabel }}
      </div>
      <template v-else>
        <slot :logs="displayLogs">
          <div
            v-for="(line, idx) in displayLogs"
            :key="idx"
            :class="wrap ? 'whitespace-pre-wrap break-all' : 'whitespace-pre'"
          >
            {{ line }}
          </div>
        </slot>
      </template>
    </div>
  </div>
</template>
