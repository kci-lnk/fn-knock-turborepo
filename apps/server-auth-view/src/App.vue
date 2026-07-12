<template>
  <div
    class="theme-grid-background fixed inset-0 -z-20 h-screen w-screen"
  ></div>
  <DynamicWhiteBackground :active="isDynamicWhiteActive" />
  <div
    class="fixed right-[calc(env(safe-area-inset-right)+1rem)] top-[calc(env(safe-area-inset-top)+1rem)] z-30"
  >
    <ThemeModeToggle />
  </div>
  <RouterView />
</template>

<script setup lang="ts">
import { computed } from "vue";
import { ThemeModeToggle, useThemeMode } from "@/components/ui/theme-toggle";
import DynamicWhiteBackground from "@admin-shared/components/appearance/DynamicWhiteBackground.vue";
import { DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY } from "@frontend-core/appearance";
import { useAppearanceState } from "@admin-shared/composables/useAppearanceState";

const { activeThemeColorPreset } = useAppearanceState();
const { resolvedMode } = useThemeMode();
const isDynamicWhiteActive = computed(
  () =>
    resolvedMode.value === "light" &&
    activeThemeColorPreset.value === DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY,
);
</script>
