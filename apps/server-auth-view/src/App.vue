<template>
  <div
    class="theme-grid-background fixed inset-0 -z-20 h-screen w-screen"
  ></div>
  <DynamicWhiteBackground v-if="isDynamicWhiteActive" :active="true" />
  <div
    class="fixed right-[calc(env(safe-area-inset-right)+1rem)] top-[calc(env(safe-area-inset-top)+1rem)] z-30"
  >
    <ThemeModeToggle />
  </div>
  <RouterView />
</template>

<script setup lang="ts">
import { computed, defineAsyncComponent, watchEffect } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute } from "vue-router";
import { ThemeModeToggle, useThemeMode } from "@/components/ui/theme-toggle";
import { DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY } from "@frontend-core/appearance";
import { useAppearanceState } from "@admin-shared/composables/useAppearanceState";

const { activeThemeColorPreset } = useAppearanceState();
const DynamicWhiteBackground = defineAsyncComponent(
  () =>
    import("@admin-shared/components/appearance/DynamicWhiteBackground.vue"),
);
const { resolvedMode } = useThemeMode();
const route = useRoute();
const { t } = useI18n();
const isDynamicWhiteActive = computed(
  () =>
    resolvedMode.value === "light" &&
    activeThemeColorPreset.value === DYNAMIC_WHITE_THEME_COLOR_PRESET_KEY,
);

watchEffect(() => {
  const title =
    route.name === "OidcBind"
      ? t("auth.oidcBind.title")
      : route.name === "NotFound"
        ? "404"
        : t("auth.title");
  document.title = `${title} · fn-knock`;
});
</script>
