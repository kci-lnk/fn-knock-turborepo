<script setup lang="ts">
import { computed, defineAsyncComponent } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useSyncedQueryTab } from "@admin-shared/composables/useSyncedQueryTab";
import { useConfigStore } from "../store/config";
import { isCloudflaredTunnelAvailable } from "../lib/reverse-proxy-submode";

const RunModeSettings = defineAsyncComponent(
  () => import("./system-settings/RunModeSettings.vue"),
);
const FrpSettings = defineAsyncComponent(
  () => import("./system-settings/FrpSettings.vue"),
);
const CloudflaredSettings = defineAsyncComponent(
  () => import("./system-settings/CloudflaredSettings.vue"),
);
const AcmeSSL = defineAsyncComponent(
  () => import("./system-settings/AcmeSSL.vue"),
);
const IpLocationSettings = defineAsyncComponent(
  () => import("./system-settings/IpLocationSettings.vue"),
);
const ScannerFirewallSettings = defineAsyncComponent(
  () => import("./system-settings/ScannerFirewallSettings.vue"),
);
const FeaturesSettings = defineAsyncComponent(
  () => import("./system-settings/FeaturesSettings.vue"),
);
const FnosSettings = defineAsyncComponent(
  () => import("./system-settings/FnosSettings.vue"),
);
const CaptchaSettings = defineAsyncComponent(
  () => import("./system-settings/CaptchaSettings.vue"),
);
const GatewayLoggingSettings = defineAsyncComponent(
  () => import("./system-settings/GatewayLoggingSettings.vue"),
);
const GatewaySettings = defineAsyncComponent(
  () => import("./system-settings/GatewaySettings.vue"),
);
const WAFSettings = defineAsyncComponent(
  () => import("./system-settings/WAFSettings.vue"),
);
const SessionSettings = defineAsyncComponent(
  () => import("./system-settings/SessionSettings.vue"),
);
const MaintenanceSettings = defineAsyncComponent(
  () => import("./system-settings/MaintenanceSettings.vue"),
);
const PanelSettings = defineAsyncComponent(
  () => import("./system-settings/PanelSettings.vue"),
);

const router = useRouter();
const route = useRoute();
const configStore = useConfigStore();
const { t } = useI18n();

const defaultTab = "run-mode";
const showFrpTab = computed(
  () => configStore.config?.run_type === 1 && configStore.canUseFrpc,
);
const showCloudflaredTab = computed(
  () =>
    configStore.config?.run_type === 1 &&
    configStore.canUseCloudflared &&
    isCloudflaredTunnelAvailable(configStore.config),
);
const showTunnelTabs = computed(
  () => showFrpTab.value || showCloudflaredTab.value,
);
const showAcmeTab = computed(
  () => configStore.canUseAcme && !configStore.isWindowsDeployment,
);
const showPanelTab = computed(
  () => configStore.isProtectedAdminPanelDeployment,
);
const showFnosTab = computed(
  () =>
    !configStore.isLinuxDeployment &&
    !configStore.isSynologyDeployment &&
    !configStore.isWindowsDeployment,
);
const allowedTabs = computed(() => {
  const tabs = [
    "run-mode",
    "acme-ssl",
    "ip-location",
    "fnos",
    "scanner-firewall",
    "features",
    "gateway",
    "waf",
    "gateway-logging",
    "session",
    "panel",
    "captcha",
    "maintenance",
  ];
  if (!showFnosTab.value) {
    const fnosIndex = tabs.indexOf("fnos");
    if (fnosIndex >= 0) tabs.splice(fnosIndex, 1);
  }
  if (!showAcmeTab.value) {
    const acmeIndex = tabs.indexOf("acme-ssl");
    if (acmeIndex >= 0) tabs.splice(acmeIndex, 1);
  }
  if (!showPanelTab.value) {
    const panelIndex = tabs.indexOf("panel");
    if (panelIndex >= 0) {
      tabs.splice(panelIndex, 1);
    }
  }
  if (showTunnelTabs.value) {
    if (showFrpTab.value) {
      tabs.splice(1, 0, "frp");
    }
    if (showCloudflaredTab.value) {
      tabs.splice(showFrpTab.value ? 2 : 1, 0, "cloudflared");
    }
  }
  return tabs;
});
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab,
  allowedTabs,
});
</script>

<template>
  <div
    class="dynamic-white-page-card dynamic-white-settings-surface h-full flex flex-col gap-4"
  >
    <Tabs
      :model-value="currentTab"
      @update:model-value="navigateTo"
      class="w-full"
    >
      <div class="w-full overflow-x-auto pb-1">
        <TabsList class="min-w-max justify-start">
          <TabsTrigger value="run-mode" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.mode")
          }}</TabsTrigger>
          <TabsTrigger
            v-if="showFrpTab"
            value="frp"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.frp") }}</TabsTrigger
          >
          <TabsTrigger
            v-if="showTunnelTabs && showCloudflaredTab"
            value="cloudflared"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.cloudflared") }}</TabsTrigger
          >
          <TabsTrigger
            v-if="showAcmeTab"
            value="acme-ssl"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.acme") }}</TabsTrigger
          >
          <TabsTrigger value="ip-location" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.ipLocation")
          }}</TabsTrigger>
          <TabsTrigger
            v-if="showFnosTab"
            value="fnos"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.fnos") }}</TabsTrigger
          >
          <TabsTrigger
            value="scanner-firewall"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.scannerFirewall") }}</TabsTrigger
          >
          <TabsTrigger value="features" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.features")
          }}</TabsTrigger>
          <TabsTrigger value="gateway" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.gateway")
          }}</TabsTrigger>
          <TabsTrigger value="waf" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.waf")
          }}</TabsTrigger>
          <TabsTrigger
            value="gateway-logging"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.logs") }}</TabsTrigger
          >
          <TabsTrigger value="session" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.session")
          }}</TabsTrigger>
          <TabsTrigger
            v-if="showPanelTab"
            value="panel"
            class="flex-none shrink-0 px-3"
            >{{ t("admin.systemSettingsTabs.panel") }}</TabsTrigger
          >
          <TabsTrigger value="captcha" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.challenge")
          }}</TabsTrigger>
          <TabsTrigger value="maintenance" class="flex-none shrink-0 px-3">{{
            t("admin.systemSettingsTabs.maintenance")
          }}</TabsTrigger>
        </TabsList>
      </div>
      <TabsContent value="run-mode" class="pt-2">
        <RunModeSettings />
      </TabsContent>
      <TabsContent v-if="showFrpTab" value="frp" class="pt-2">
        <FrpSettings />
      </TabsContent>
      <TabsContent
        v-if="showTunnelTabs && showCloudflaredTab"
        value="cloudflared"
        class="pt-2"
      >
        <CloudflaredSettings />
      </TabsContent>
      <TabsContent v-if="showAcmeTab" value="acme-ssl" class="pt-2">
        <AcmeSSL />
      </TabsContent>
      <TabsContent value="ip-location" class="pt-2">
        <IpLocationSettings />
      </TabsContent>
      <TabsContent v-if="showFnosTab" value="fnos" class="pt-2">
        <FnosSettings />
      </TabsContent>
      <TabsContent value="scanner-firewall" class="pt-2">
        <ScannerFirewallSettings />
      </TabsContent>
      <TabsContent value="features" class="pt-2">
        <FeaturesSettings />
      </TabsContent>
      <TabsContent value="gateway" class="pt-2">
        <GatewaySettings />
      </TabsContent>
      <TabsContent value="waf" class="pt-2">
        <WAFSettings />
      </TabsContent>
      <TabsContent value="gateway-logging" class="pt-2">
        <GatewayLoggingSettings />
      </TabsContent>
      <TabsContent value="session" class="pt-2">
        <SessionSettings />
      </TabsContent>
      <TabsContent v-if="showPanelTab" value="panel" class="pt-2">
        <PanelSettings />
      </TabsContent>
      <TabsContent value="captcha" class="pt-2">
        <CaptchaSettings />
      </TabsContent>
      <TabsContent value="maintenance" class="pt-2">
        <MaintenanceSettings />
      </TabsContent>
    </Tabs>
  </div>
</template>
