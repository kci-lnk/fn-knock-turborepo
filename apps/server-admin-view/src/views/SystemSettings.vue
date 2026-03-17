<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import RunModeSettings from './system-settings/RunModeSettings.vue'
import FrpSettings from './system-settings/FrpSettings.vue'
import CloudflaredSettings from './system-settings/CloudflaredSettings.vue'
import AcmeSSL from './system-settings/AcmeSSL.vue'
import ScannerFirewallSettings from './system-settings/ScannerFirewallSettings.vue'
import FnosSettings from './system-settings/FnosSettings.vue'
import CaptchaSettings from './system-settings/CaptchaSettings.vue'
import { useSyncedQueryTab } from '@admin-shared/composables/useSyncedQueryTab'

const router = useRouter()
const route = useRoute()

const defaultTab = 'run-mode'
const allowedTabs = new Set([
  'run-mode',
  'frp',
  'cloudflared',
  'acme-ssl',
  'fnos',
  'scanner-firewall',
  'captcha',
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
      <div class="w-full overflow-x-auto pb-1">
        <TabsList class="min-w-max justify-start">
          <TabsTrigger value="run-mode" class="flex-none shrink-0 px-3">模式</TabsTrigger>
          <TabsTrigger value="frp" class="flex-none shrink-0 px-3">FRP</TabsTrigger>
          <TabsTrigger value="cloudflared" class="flex-none shrink-0 px-3">Cloudflared</TabsTrigger>
          <TabsTrigger value="acme-ssl" class="flex-none shrink-0 px-3">ACME</TabsTrigger>
          <TabsTrigger value="fnos" class="flex-none shrink-0 px-3">飞牛</TabsTrigger>
          <TabsTrigger value="scanner-firewall" class="flex-none shrink-0 px-3">拦截</TabsTrigger>
          <TabsTrigger value="captcha" class="flex-none shrink-0 px-3">验证码</TabsTrigger>
        </TabsList>
      </div>
      <TabsContent value="run-mode" class="pt-2">
        <RunModeSettings />
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
      <TabsContent value="fnos" class="pt-2">
        <FnosSettings />
      </TabsContent>
      <TabsContent value="scanner-firewall" class="pt-2">
        <ScannerFirewallSettings />
      </TabsContent>
      <TabsContent value="captcha" class="pt-2">
        <CaptchaSettings />
      </TabsContent>
    </Tabs>
  </div>
</template>
