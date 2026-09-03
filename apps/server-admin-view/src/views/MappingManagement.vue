<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { isProtocolMappingVisible } from "@/lib/protocol-mapping-visibility";
import { useConfigStore } from "@/store/config";

const SubdomainTab = defineAsyncComponent(() => import("./SubdomainProxy.vue"));
const ProtocolTab = defineAsyncComponent(() => import("./StreamMappings.vue"));

const route = useRoute();
const router = useRouter();
const configStore = useConfigStore();
const { t } = useI18n();

const showProtocolTab = computed(() =>
  isProtocolMappingVisible(configStore.config),
);
const allowedTabs = computed(() =>
  showProtocolTab.value ? ["subdomain", "protocol"] : ["subdomain"],
);
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab: "subdomain",
  allowedTabs,
});
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface h-full flex flex-col gap-4"
  >
    <Tabs
      :model-value="currentTab"
      class="w-full"
      @update:model-value="navigateTo"
    >
      <div
        class="grid gap-2 sm:grid-cols-[auto_minmax(0,1fr)] sm:items-center sm:gap-x-3 sm:gap-y-1"
      >
        <h2 class="text-lg font-semibold tracking-tight">
          {{ t("admin.mappingManagement.title") }}
        </h2>
        <p
          class="order-2 text-sm text-muted-foreground sm:col-span-2 sm:row-start-2"
        >
          {{ t("admin.mappingManagement.description") }}
        </p>
        <div
          class="order-3 min-w-0 overflow-x-auto pb-1 sm:order-none sm:col-start-2 sm:row-start-1 sm:justify-self-start sm:pb-0"
        >
          <TabsList>
            <TabsTrigger value="subdomain">
              {{ t("admin.mappingManagement.subdomainTab") }}
            </TabsTrigger>
            <TabsTrigger v-if="showProtocolTab" value="protocol">
              {{ t("admin.mappingManagement.protocolTab") }}
            </TabsTrigger>
          </TabsList>
        </div>
      </div>

      <TabsContent value="subdomain" class="pt-2">
        <SubdomainTab />
      </TabsContent>
      <TabsContent v-if="showProtocolTab" value="protocol" class="pt-2">
        <ProtocolTab />
      </TabsContent>
    </Tabs>
  </div>
</template>
