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
    <div class="space-y-1">
      <h2 class="text-lg font-semibold tracking-tight">
        {{ t("admin.mappingManagement.title") }}
      </h2>
      <p class="text-sm text-muted-foreground">
        {{ t("admin.mappingManagement.description") }}
      </p>
    </div>

    <Tabs
      :model-value="currentTab"
      class="w-full"
      @update:model-value="navigateTo"
    >
      <div class="overflow-x-auto pb-1">
        <TabsList>
          <TabsTrigger value="subdomain">
            {{ t("admin.mappingManagement.subdomainTab") }}
          </TabsTrigger>
          <TabsTrigger v-if="showProtocolTab" value="protocol">
            {{ t("admin.mappingManagement.protocolTab") }}
          </TabsTrigger>
        </TabsList>
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
