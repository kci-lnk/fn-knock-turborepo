<template>
  <div class="h-full flex flex-col gap-4">
    <Tabs :model-value="currentTab" @update:model-value="navigateTo" class="w-full">
      <TabsList data-guide-ssl-settings>
        <TabsTrigger value="cert-config">证书配置</TabsTrigger>
        <TabsTrigger value="self-signed">自签证书</TabsTrigger>
        <TabsTrigger value="acme-cert">ACME证书</TabsTrigger>
      </TabsList>
      <TabsContent value="cert-config" class="pt-2">
        <CertConfig />
      </TabsContent>
      <TabsContent value="self-signed" class="pt-2">
        <SelfSignedCA />
      </TabsContent>
      <TabsContent value="acme-cert" class="pt-2">
        <AcmeCert />
      </TabsContent>
    </Tabs>
  </div>
</template>

<script setup lang="ts">
import { useRoute, useRouter } from 'vue-router'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import CertConfig from './ssl-settings/CertConfig.vue'
import SelfSignedCA from './ssl-settings/SelfSignedCA.vue'
import AcmeCert from './ssl-settings/AcmeCert.vue'
import { useSyncedQueryTab } from '@admin-shared/composables/useSyncedQueryTab'

const router = useRouter()
const route = useRoute()

const defaultTab = 'cert-config'
const allowedTabs = new Set([defaultTab, 'self-signed', 'acme-cert'])
const { currentTab, navigateTo } = useSyncedQueryTab({
  route,
  router,
  defaultTab,
  allowedTabs,
})
</script>
