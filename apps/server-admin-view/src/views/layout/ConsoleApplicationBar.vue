<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { AppWindow, Globe2, Link2 } from "lucide-vue-next";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { useConfigStore } from "@/store/config";
import {
  buildConsoleApplicationItems,
  shouldShowConsoleApplicationList,
} from "./console-application-list";

const configStore = useConfigStore();
const { t } = useI18n();
const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();
const brokenIcons = ref<Set<string>>(new Set());
const isVisible = computed(() =>
  shouldShowConsoleApplicationList({
    deploymentTarget: configStore.runtimeProfile?.deployment_target,
    enabled:
      configStore.config?.dashboard_display?.show_console_app_list === true,
  }),
);

const items = computed(() => {
  if (!isVisible.value || !configStore.config || typeof window === "undefined")
    return [];
  return buildConsoleApplicationItems({
    accessEntryPort: accessEntryPort.value,
    config: configStore.config,
    location: window.location,
  });
});

const markIconBroken = (key: string) => {
  const next = new Set(brokenIcons.value);
  next.add(key);
  brokenIcons.value = next;
};
const iconFailureKey = (key: string, iconSrc: string) =>
  `${key}\u0000${iconSrc}`;

watch(
  isVisible,
  (visible) => {
    if (visible) void loadAccessEntryPort();
  },
  { immediate: true },
);
</script>

<template>
  <nav
    v-if="isVisible"
    :aria-label="t('admin.consoleApplicationList.ariaLabel')"
    class="mb-4 flex w-full max-w-full min-w-0 items-center gap-2 overflow-hidden rounded-lg border border-border/55 bg-muted/15 px-2.5 py-1.5 shadow-none"
  >
    <div
      class="flex h-8 shrink-0 items-center gap-1.5 border-r border-border/50 pr-2.5 text-xs font-medium text-muted-foreground/85"
    >
      <AppWindow class="h-3.5 w-3.5" aria-hidden="true" />
      <span>{{ t("admin.consoleApplicationList.label") }}</span>
    </div>

    <ul
      v-if="items.length > 0"
      class="flex min-w-0 flex-1 snap-x items-center gap-1.5 overflow-x-auto overscroll-x-contain [scrollbar-width:none] [-ms-overflow-style:none] [&::-webkit-scrollbar]:hidden"
    >
      <li v-for="item in items" :key="item.key" class="shrink-0 snap-start">
        <a
          :href="item.href"
          target="_blank"
          rel="noopener noreferrer"
          class="group inline-flex h-8 max-w-48 items-center gap-2 rounded-md px-2 text-xs font-medium text-foreground/85 transition-colors hover:bg-muted/55 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-inset"
          :title="item.label"
          :aria-label="
            t('admin.consoleApplicationList.openApplication', {
              name: item.label,
            })
          "
        >
          <span
            v-if="item.showIcon"
            class="grid h-5 w-5 shrink-0 place-items-center overflow-hidden rounded text-muted-foreground/80"
          >
            <img
              v-if="
                item.iconSrc &&
                !brokenIcons.has(iconFailureKey(item.key, item.iconSrc))
              "
              :src="item.iconSrc"
              alt=""
              class="h-full w-full object-contain p-0.5"
              @error="markIconBroken(iconFailureKey(item.key, item.iconSrc))"
            />
            <Globe2
              v-else-if="item.kind === 'host'"
              class="h-3.5 w-3.5"
              aria-hidden="true"
            />
            <Link2 v-else class="h-3.5 w-3.5" aria-hidden="true" />
          </span>
          <span class="min-w-0 truncate">{{ item.label }}</span>
        </a>
      </li>
    </ul>

    <p v-else class="min-w-0 flex-1 truncate text-xs text-muted-foreground">
      {{ t("admin.consoleApplicationList.empty") }}
    </p>
  </nav>
</template>
