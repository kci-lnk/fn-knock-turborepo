<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  RekaTabs,
  RekaTabsContent,
  RekaTabsList,
  RekaTabsTrigger,
} from "@/components/reka-tabs";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import ProvidersTab from "./notifications/ProvidersTab.vue";
import RulesTab from "./notifications/RulesTab.vue";
import DeliveriesTab from "./notifications/DeliveriesTab.vue";

const notificationTabs = [
  {
    value: "providers",
    labelKey: "admin.notifications.tabs.providers",
  },
  {
    value: "rules",
    labelKey: "admin.notifications.tabs.rules",
  },
  {
    value: "deliveries",
    labelKey: "admin.notifications.tabs.deliveries",
  },
] as const;

type NotificationTabValue = (typeof notificationTabs)[number]["value"];

const { t } = useI18n();
const router = useRouter();
const route = useRoute();

const allowedTabs = notificationTabs.map((tab) => tab.value);
const isNotificationsRouteActive = computed(
  () => String(route.query.tab || "events") === "notifications",
);

const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab: "providers",
  allowedTabs,
  queryKey: "notificationsTab",
  active: isNotificationsRouteActive,
});

const visitedTabs = ref<NotificationTabValue[]>(["providers"]);

watch(currentTab, (nextTab) => {
  const normalizedTab = nextTab as NotificationTabValue;
  if (visitedTabs.value.includes(normalizedTab)) return;
  visitedTabs.value = [...visitedTabs.value, normalizedTab];
});
</script>

<template>
  <div class="min-h-0">
    <RekaTabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      :unmount-on-hide="false"
      class="min-h-0 gap-0 overflow-hidden rounded-xl border border-border/70 bg-card shadow-sm"
    >
      <div class="border-b border-border/70 bg-muted/20 px-4 sm:px-6">
        <RekaTabsList
          :aria-label="t('admin.notifications.tabs.aria')"
          class="w-fit rounded-none border-0 bg-transparent px-0 after:right-0 after:left-0 after:bg-border/70"
          indicator-class="h-px bg-foreground"
        >
          <RekaTabsTrigger
            v-for="tab in notificationTabs"
            :key="tab.value"
            :value="tab.value"
            class="min-w-[108px] px-5 text-muted-foreground transition-colors hover:bg-transparent hover:text-foreground focus-visible:ring-ring/30 data-[state=active]:font-semibold data-[state=active]:text-foreground"
          >
            {{ t(tab.labelKey) }}
          </RekaTabsTrigger>
        </RekaTabsList>
      </div>

      <RekaTabsContent value="providers" class="min-h-0">
        <ProvidersTab
          v-if="visitedTabs.includes('providers')"
          :active="currentTab === 'providers'"
        />
      </RekaTabsContent>

      <RekaTabsContent value="rules" class="min-h-0">
        <RulesTab
          v-if="visitedTabs.includes('rules')"
          :active="currentTab === 'rules'"
        />
      </RekaTabsContent>

      <RekaTabsContent value="deliveries" class="min-h-0">
        <DeliveriesTab
          v-if="visitedTabs.includes('deliveries')"
          :active="currentTab === 'deliveries'"
        />
      </RekaTabsContent>
    </RekaTabs>
  </div>
</template>
