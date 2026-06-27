<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import EventsTab from "./event-center/EventsTab.vue";
import NotificationsTab from "./event-center/NotificationsTab.vue";

const { t } = useI18n();
const router = useRouter();
const route = useRoute();

const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab: "events",
  allowedTabs: ["events", "notifications"],
});
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-4"
  >
    <div class="space-y-1">
      <div class="text-xl font-semibold tracking-tight text-foreground">
        {{ t("admin.eventCenter.title") }}
      </div>
      <div class="text-sm leading-6 text-muted-foreground">
        {{ t("admin.eventCenter.description") }}
      </div>
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
      </TabsList>

      <TabsContent value="events" class="min-h-0 flex-1 pt-2">
        <EventsTab :active="currentTab === 'events'" />
      </TabsContent>

      <TabsContent value="notifications" class="min-h-0 flex-1 pt-2">
        <NotificationsTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
