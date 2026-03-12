<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import RunModeSettings from './system-settings/RunModeSettings.vue'
import IpLocationSettings from './system-settings/IpLocationSettings.vue'
import FrpSettings from './system-settings/FrpSettings.vue'
import CloudflaredSettings from './system-settings/CloudflaredSettings.vue'
import AcmeSSL from './system-settings/AcmeSSL.vue'
import ScannerFirewallSettings from './system-settings/ScannerFirewallSettings.vue'
import { useSyncedQueryTab } from '@admin-shared/composables/useSyncedQueryTab'

const router = useRouter()
const route = useRoute()

const defaultTab = 'run-mode'
const allowedTabs = new Set([
  'run-mode',
  'ip-location',
  'frp',
  'cloudflared',
  'acme-ssl',
  'scanner-firewall',
])
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
      <TabsList>
        <TabsTrigger value="run-mode">模式</TabsTrigger>
        <TabsTrigger value="ip-location">IP归属</TabsTrigger>
        <TabsTrigger value="frp">FRP</TabsTrigger>
        <TabsTrigger value="cloudflared">Cloudflared</TabsTrigger>
        <TabsTrigger value="acme-ssl">ACME</TabsTrigger>
        <TabsTrigger value="scanner-firewall">扫描拦截</TabsTrigger>
      </TabsList>
      <TabsContent value="run-mode" class="pt-2">
        <RunModeSettings />
      </TabsContent>
      <TabsContent value="ip-location" class="pt-2">
        <IpLocationSettings />
      </TabsContent>
      <TabsContent value="frp" class="pt-2">
        <FrpSettings />
      </TabsContent>
      <TabsContent value="cloudflared" class="pt-2">
        <CloudflaredSettings />
      </TabsContent>
      <TabsContent value="acme-ssl" class="pt-2">
        <AcmeSSL />
      </TabsContent>
      <TabsContent value="scanner-firewall" class="pt-2">
        <ScannerFirewallSettings />
      </TabsContent>
    </Tabs>
  </div>
</template>
