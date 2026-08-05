<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import type { AnalyticsBreakdownItem } from "./model";
import { formatAnalyticsNumber, formatAnalyticsPercent } from "./model";

interface BreakdownTab {
  key: string;
  label: string;
  items: AnalyticsBreakdownItem[];
  metricLabel?: string;
  footer?: string;
}

const props = defineProps<{
  title: string;
  tabs: BreakdownTab[];
  emptyText: string;
  defaultMetricLabel: string;
  footer?: string;
}>();

const { locale } = useI18n();
const activeKey = ref(props.tabs[0]?.key || "");
watch(
  () => props.tabs.map((tab) => tab.key).join("|"),
  () => {
    if (!props.tabs.some((tab) => tab.key === activeKey.value)) {
      activeKey.value = props.tabs[0]?.key || "";
    }
  },
);
const activeTab = computed(
  () => props.tabs.find((tab) => tab.key === activeKey.value) || props.tabs[0],
);
const metricLabel = computed(
  () => activeTab.value?.metricLabel || props.defaultMetricLabel,
);
const footerText = computed(
  () => activeTab.value?.footer || props.footer || "",
);

const barWidth = (share: number) =>
  `${Math.max(0, Math.min(100, share * 100))}%`;
</script>

<template>
  <Card class="min-w-0 overflow-hidden shadow-none">
    <Tabs
      :model-value="activeKey"
      class="gap-0"
      @update:model-value="activeKey = String($event)"
    >
      <CardHeader class="border-b px-3 py-3 sm:px-4">
        <div class="flex items-center justify-between gap-3">
          <CardTitle class="text-sm font-medium">{{ props.title }}</CardTitle>
          <div class="flex shrink-0 items-center gap-2">
            <span
              class="text-[10px] font-medium uppercase tracking-wide text-muted-foreground"
            >
              {{ metricLabel }}
            </span>
            <slot name="action" />
          </div>
        </div>
        <TabsList
          class="analytics-breakdown-tablist h-8 max-w-full justify-start overflow-x-auto bg-transparent p-0"
        >
          <TabsTrigger
            v-for="tab in props.tabs"
            :key="tab.key"
            :value="tab.key"
            class="analytics-breakdown-tab h-8 shrink-0 bg-transparent px-2.5 text-xs"
          >
            {{ tab.label }}
          </TabsTrigger>
        </TabsList>
      </CardHeader>
      <CardContent class="p-2 sm:p-3">
        <TabsContent
          v-for="tab in props.tabs"
          :key="tab.key"
          :value="tab.key"
          class="mt-0 outline-none"
        >
          <div
            v-if="tab.items.length"
            class="space-y-1"
            role="list"
            :aria-label="`${props.title} · ${tab.label}`"
          >
            <div
              v-for="item in tab.items"
              :key="item.key"
              class="relative flex min-h-10 items-center gap-2 overflow-hidden rounded-md px-2 py-2 text-[13px] outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-1 sm:gap-3 sm:px-2.5 sm:text-sm"
              role="listitem"
              tabindex="0"
              :aria-label="`${item.label}: ${formatAnalyticsNumber(item.count, String(locale))}, ${formatAnalyticsPercent(item.share, String(locale))}`"
            >
              <span
                class="absolute inset-y-0 left-0 rounded-md bg-foreground/[0.08] transition-[width] duration-300 dark:bg-foreground/[0.14]"
                :class="{ 'min-w-1': item.share > 0 }"
                :style="{ width: barWidth(item.share) }"
                aria-hidden="true"
              />
              <span
                class="relative min-w-0 flex-1 truncate"
                :title="item.label"
              >
                {{ item.label }}
              </span>
              <span
                class="relative flex shrink-0 items-baseline gap-2 tabular-nums"
              >
                <span class="font-medium">
                  {{ formatAnalyticsNumber(item.count, String(locale)) }}
                </span>
                <span class="text-xs text-muted-foreground">
                  {{ formatAnalyticsPercent(item.share, String(locale)) }}
                </span>
              </span>
            </div>
          </div>
          <div
            v-else
            class="flex min-h-40 items-center justify-center px-4 text-center text-xs text-muted-foreground"
          >
            {{ props.emptyText }}
          </div>
        </TabsContent>
        <p
          v-if="footerText"
          class="mt-3 text-[11px] leading-4 text-muted-foreground"
        >
          {{ footerText }}
        </p>
      </CardContent>
    </Tabs>
  </Card>
</template>

<style scoped>
:deep(.analytics-breakdown-tablist) {
  border-radius: 0;
  scrollbar-width: none;
}

:deep(.analytics-breakdown-tablist::-webkit-scrollbar) {
  display: none;
}

:deep(.analytics-breakdown-tab) {
  flex: none;
  border-width: 0 0 2px;
  border-color: transparent !important;
  border-radius: 0;
  background: transparent !important;
  box-shadow: none !important;
}

:deep(.analytics-breakdown-tab[data-state="active"]) {
  border-bottom-color: currentColor !important;
}
</style>
