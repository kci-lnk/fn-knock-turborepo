<script setup lang="ts">
import { computed, defineAsyncComponent, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useConfigStore } from "../store/config";
import DocsLinkButton from "@/components/DocsLinkButton.vue";
import { docsUrls } from "../lib/docs";
import { isCloudflaredTunnelAvailable } from "../lib/reverse-proxy-submode";

const FrpTunnel = defineAsyncComponent(() => import("./tunnel/FrpTunnel.vue"));
const CloudflareTunnel = defineAsyncComponent(
  () => import("./tunnel/CloudflareTunnel.vue"),
);

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const { t } = useI18n();

const defaultTunnel = ref<string>("frp");
const allowedTabs = computed(() => [
  ...(configStore.canUseFrpc ? ["frp"] : []),
  ...(configStore.canUseCloudflared &&
  isCloudflaredTunnelAvailable(configStore.config)
    ? ["cloudflared"]
    : []),
]);
const showTabSwitcher = computed(() => allowedTabs.value.length > 1);
const isInitialized = ref(false);
const { isPending: isLoading, run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    console.error(
      "Failed to load tunnel config:",
      extractErrorMessage(error, t("admin.tunnel.loadConfigFailed")),
    );
  },
});

function resolveDefaultTunnel(value: string | null | undefined) {
  const availableTabs = new Set(allowedTabs.value);
  return value && availableTabs.has(value)
    ? value
    : (allowedTabs.value[0] ?? "frp");
}

watch(
  () => [
    configStore.config?.run_type,
    configStore.config?.reverse_proxy_submode,
  ],
  ([runType]) => {
    if (runType !== undefined && runType !== 1) {
      void router.replace({ path: "/system" });
    }
  },
);

watch(
  () => configStore.config?.default_tunnel,
  (value) => {
    defaultTunnel.value = resolveDefaultTunnel(value);
  },
  { immediate: true },
);

const { currentTab, navigateTo, syncFromRoute } = useSyncedQueryTab({
  route,
  router,
  defaultTab: defaultTunnel,
  allowedTabs,
  active: () => isInitialized.value,
});

async function loadConfig() {
  await runLoadConfig(async () => {
    if (!configStore.config) {
      await configStore.loadConfig();
    }
    const config = configStore.config;
    if (!config || config.run_type !== 1 || allowedTabs.value.length === 0) {
      await router.replace({ path: "/system" });
      return;
    }
  });
  if (route.path !== "/tunnel") {
    return;
  }
  isInitialized.value = true;
  syncFromRoute();
}

onMounted(() => {
  loadConfig();
});
</script>

<template>
  <div v-if="isInitialized && !isLoading" class="h-full flex flex-col gap-4">
    <Tabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      class="w-full"
    >
      <div
        class="flex items-center w-full"
        :class="showTabSwitcher ? 'justify-between' : 'justify-end'"
      >
        <TabsList v-if="showTabSwitcher">
          <TabsTrigger value="frp">FRP</TabsTrigger>
          <TabsTrigger
            v-if="allowedTabs.includes('cloudflared')"
            value="cloudflared"
          >
            Cloudflared
          </TabsTrigger>
        </TabsList>
        <DocsLinkButton v-if="showTabSwitcher" :href="docsUrls.guides.tunnel" />
      </div>

      <TabsContent v-if="configStore.canUseFrpc" value="frp" class="pt-2">
        <FrpTunnel :show-docs-button="!showTabSwitcher" />
      </TabsContent>
      <TabsContent
        v-if="allowedTabs.includes('cloudflared')"
        value="cloudflared"
        class="pt-2"
      >
        <CloudflareTunnel />
      </TabsContent>
    </Tabs>
  </div>
</template>
