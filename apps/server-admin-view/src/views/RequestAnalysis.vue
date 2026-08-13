<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Info, Settings } from "lucide-vue-next";
import { Alert } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useConfigStore } from "@/store/config";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";

const GatewayRequestLogs = defineAsyncComponent(
  () => import("./GatewayRequestLogs.vue"),
);
const RequestAnalyticsTab = defineAsyncComponent(
  () => import("./request-analysis/RequestAnalyticsTab.vue"),
);

const { t } = useI18n();
const route = useRoute();
const router = useRouter();
const configStore = useConfigStore();
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab: "logs",
  allowedTabs: ["logs", "analytics"],
});
const isLoggingEnabled = computed(
  () => configStore.config?.gateway_logging?.enabled ?? false,
);

const goToSettings = () => {
  void router.push({ path: "/system", query: { tab: "gateway-logging" } });
};
</script>

<template>
  <div
    class="request-analysis-page dynamic-white-page-card dynamic-white-settings-surface flex h-full flex-col gap-3 sm:gap-4"
  >
    <div class="space-y-1">
      <h2 class="text-lg font-semibold tracking-tight">
        {{ t("admin.requestAnalysis.title") }}
      </h2>
      <p class="text-sm text-muted-foreground">
        {{ t("admin.requestAnalysis.description") }}
      </p>
    </div>

    <Alert
      v-if="!isLoggingEnabled"
      class="flex items-center gap-3 rounded-lg border-dashed bg-muted/20 px-4 py-3 text-foreground shadow-none"
    >
      <Info class="h-4 w-4 shrink-0 text-muted-foreground" />
      <div
        class="flex w-full flex-col gap-2 sm:flex-row sm:items-center sm:justify-between"
      >
        <p class="text-sm text-muted-foreground">
          {{ t("admin.gatewayRequestLogs.disabledNotice") }}
        </p>
        <Button variant="ghost" class="shrink-0" @click="goToSettings">
          <Settings class="mr-2 h-4 w-4" />
          {{ t("admin.gatewayRequestLogs.goSettings") }}
        </Button>
      </div>
    </Alert>

    <Tabs
      :model-value="currentTab"
      :unmount-on-hide="false"
      class="min-h-0 w-full flex-1"
      @update:model-value="navigateTo"
    >
      <div
        class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between"
      >
        <TabsList class="grid w-full shrink-0 grid-cols-2 sm:flex sm:w-auto">
          <TabsTrigger value="logs">
            {{ t("admin.requestAnalysis.pageTabs.logs") }}
          </TabsTrigger>
          <TabsTrigger value="analytics">
            {{ t("admin.requestAnalysis.pageTabs.analytics") }}
          </TabsTrigger>
        </TabsList>
        <div
          class="flex min-h-9 w-full min-w-0 items-center justify-end sm:w-auto"
        >
          <div
            v-show="currentTab === 'logs'"
            id="request-analysis-logs-actions"
            class="w-full sm:w-auto"
          />
          <div
            v-show="currentTab === 'analytics'"
            id="request-analysis-analytics-actions"
            class="w-full sm:w-auto"
          />
        </div>
      </div>
      <div class="min-h-0 pt-3">
        <TabsContent value="logs" class="mt-0 outline-none">
          <KeepAlive>
            <GatewayRequestLogs v-if="currentTab === 'logs'" />
          </KeepAlive>
        </TabsContent>
        <TabsContent value="analytics" class="mt-0 outline-none">
          <KeepAlive>
            <RequestAnalyticsTab v-if="currentTab === 'analytics'" />
          </KeepAlive>
        </TabsContent>
      </div>
    </Tabs>
  </div>
</template>

<style scoped>
@media (max-width: 639px) {
  :global(:root:not(.dark)[data-theme-color="dynamic_white"])
    .request-analysis-page {
    border-radius: 1rem;
    padding: 0.875rem;
  }
}
</style>
