<template>
  <a
    href="#main-content"
    class="sr-only fixed left-4 top-4 z-[100] rounded-md bg-background px-4 py-2 text-sm font-medium text-foreground shadow-lg focus:not-sr-only focus:outline-none focus:ring-2 focus:ring-ring"
    @click.prevent="focusMainContent"
  >
    {{ t("admin.nav.skipToContent") }}
  </a>
</template>

<script setup lang="ts">
import { nextTick, watch } from "vue";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  routePath: string;
  pageLabel: string;
  isLite: boolean;
}>();

const { t } = useI18n();

const focusMainContent = () => {
  document.getElementById("main-content")?.focus({ preventScroll: true });
};

watch(
  () => props.routePath,
  () => void nextTick(focusMainContent),
);

watch(
  () => [props.pageLabel, props.isLite] as const,
  ([label, isLite]) => {
    document.title = `${label} · ${isLite ? "fn-knock Lite" : "fn-knock"}`;
  },
  { immediate: true },
);
</script>
