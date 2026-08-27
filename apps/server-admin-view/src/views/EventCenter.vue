<script setup lang="ts">
import { defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import TraceLookupButton from "@/components/TraceLookupButton.vue";

const EventsTab = defineAsyncComponent(
  () => import("./event-center/EventsTab.vue"),
);
const NotificationsTab = defineAsyncComponent(
  () => import("./event-center/NotificationsTab.vue"),
);
const RuntimeTab = defineAsyncComponent(
  () => import("./event-center/RuntimeTab.vue"),
);

const { t } = useI18n();
const router = useRouter();
const route = useRoute();

const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab: "events",
  allowedTabs: ["events", "runtime", "notifications"],
});
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-4"
  >
    <div
      class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
    >
      <div class="space-y-1">
        <div class="text-xl font-semibold tracking-tight text-foreground">
          {{ t("admin.eventCenter.title") }}
        </div>
        <div class="text-sm leading-6 text-muted-foreground">
          {{ t("admin.eventCenter.description") }}
        </div>
      </div>
      <TraceLookupButton />
    </div>

    <Tabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      class="flex flex-1 flex-col"
    >
      <TabsList class="w-fit">
        <TabsTrigger value="events">
          {{ t("admin.eventCenter.tabs.events") }}
        </TabsTrigger>
        <TabsTrigger value="notifications">
          {{ t("admin.eventCenter.tabs.notifications") }}
        </TabsTrigger>
        <TabsTrigger value="runtime">
          {{ t("admin.eventCenter.tabs.runtime") }}
        </TabsTrigger>
      </TabsList>

      <TabsContent value="events" class="min-h-0 flex-1 pt-2">
        <EventsTab :active="currentTab === 'events'" />
      </TabsContent>

      <TabsContent value="notifications" class="min-h-0 flex-1 pt-2">
        <NotificationsTab />
      </TabsContent>

      <TabsContent value="runtime" class="min-h-0 flex-1 pt-2">
        <RuntimeTab :active="currentTab === 'runtime'" />
      </TabsContent>
    </Tabs>
  </div>
</template>
