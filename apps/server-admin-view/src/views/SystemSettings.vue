<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import RunModeSettings from './system-settings/RunModeSettings.vue'
import FrpSettings from './system-settings/FrpSettings.vue'
import CloudflaredSettings from './system-settings/CloudflaredSettings.vue'
import AcmeSSL from './system-settings/AcmeSSL.vue'
import ScannerFirewallSettings from './system-settings/ScannerFirewallSettings.vue'
import FnosSettings from './system-settings/FnosSettings.vue'
import CaptchaSettings from './system-settings/CaptchaSettings.vue'
import GatewayLoggingSettings from './system-settings/GatewayLoggingSettings.vue'
import { useSyncedQueryTab } from '@admin-shared/composables/useSyncedQueryTab'
import { useConfigStore } from '../store/config'

const router = useRouter()
const route = useRoute()
const configStore = useConfigStore()

const defaultTab = 'run-mode'
const showTunnelTabs = computed(() => configStore.config?.run_type === 1)
const allowedTabs = computed(() => {
  const tabs = [
    'run-mode',
    'acme-ssl',
    'fnos',
    'scanner-firewall',
    'gateway-logging',
    'captcha',
  ]
  if (showTunnelTabs.value) {
    tabs.splice(1, 0, 'frp', 'cloudflared')
  }
  return tabs
})
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab,
  allowedTabs,
})
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <Tabs :model-value="currentTab" @update:model-value="navigateTo" class="w-full">
      <div class="w-full overflow-x-auto pb-1">
        <TabsList class="min-w-max justify-start">
          <TabsTrigger value="run-mode" class="flex-none shrink-0 px-3">模式</TabsTrigger>
          <TabsTrigger v-if="showTunnelTabs" value="frp" class="flex-none shrink-0 px-3">FRP</TabsTrigger>
          <TabsTrigger v-if="showTunnelTabs" value="cloudflared" class="flex-none shrink-0 px-3">Cloudflared</TabsTrigger>
          <TabsTrigger value="acme-ssl" class="flex-none shrink-0 px-3">ACME</TabsTrigger>
          <TabsTrigger value="fnos" class="flex-none shrink-0 px-3">飞牛</TabsTrigger>
          <TabsTrigger value="scanner-firewall" class="flex-none shrink-0 px-3">拦截</TabsTrigger>
          <TabsTrigger value="gateway-logging" class="flex-none shrink-0 px-3">日志</TabsTrigger>
          <TabsTrigger value="captcha" class="flex-none shrink-0 px-3">验证码</TabsTrigger>
        </TabsList>
      </div>
      <TabsContent value="run-mode" class="pt-2">
        <RunModeSettings />
      </TabsContent>
      <TabsContent v-if="showTunnelTabs" value="frp" class="pt-2">
        <FrpSettings />
      </TabsContent>
      <TabsContent v-if="showTunnelTabs" value="cloudflared" class="pt-2">
        <CloudflaredSettings />
      </TabsContent>
      <TabsContent value="acme-ssl" class="pt-2">
        <AcmeSSL />
      </TabsContent>
      <TabsContent value="fnos" class="pt-2">
        <FnosSettings />
      </TabsContent>
      <TabsContent value="scanner-firewall" class="pt-2">
        <ScannerFirewallSettings />
      </TabsContent>
      <TabsContent value="gateway-logging" class="pt-2">
        <GatewayLoggingSettings />
      </TabsContent>
      <TabsContent value="captcha" class="pt-2">
        <CaptchaSettings />
      </TabsContent>
    </Tabs>
  </div>
</template>
