<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { useSyncedQueryTab } from '@admin-shared/composables/useSyncedQueryTab'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import FrpTunnel from './tunnel/FrpTunnel.vue'
import CloudflareTunnel from './tunnel/CloudflareTunnel.vue'
import { useConfigStore } from '../store/config'

const router = useRouter()
const route = useRoute()
const configStore = useConfigStore()

const defaultTunnel = ref<string>('frp')
const allowedTabs = new Set(['frp', 'cloudflared'])
const isInitialized = ref(false)
const { isPending: isLoading, run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    console.error('Failed to load tunnel config:', extractErrorMessage(error, '加载隧道配置失败'))
  },
})

watch(
  () => configStore.config?.run_type,
  (runType) => {
    if (runType !== undefined && runType !== 1) {
      void router.replace({ path: '/system' })
    }
  },
)

const { currentTab, navigateTo, syncFromRoute } = useSyncedQueryTab({
  route,
  router,
  defaultTab: defaultTunnel,
  allowedTabs,
  active: () => isInitialized.value,
})

async function loadConfig() {
  await runLoadConfig(async () => {
    if (!configStore.config) {
      await configStore.loadConfig()
    }
    const config = configStore.config
    if (!config || config.run_type !== 1) {
      await router.replace({ path: '/system' })
      return
    }
    const rawDefault = config.default_tunnel || 'frp'
    const def = allowedTabs.has(rawDefault) ? rawDefault : 'frp'
    defaultTunnel.value = def
  })
  if (route.path !== '/tunnel') {
    return
  }
  isInitialized.value = true
  syncFromRoute()
}

onMounted(() => {
  loadConfig()
})
</script>

<template>
  <div v-if="isInitialized && !isLoading" class="h-full flex flex-col gap-4">
    <Tabs :model-value="currentTab" @update:model-value="navigateTo" class="w-full">
      <div class="flex items-center justify-between w-full">
        <TabsList>
          <TabsTrigger value="frp">FRP</TabsTrigger>
          <TabsTrigger value="cloudflared">Cloudflared</TabsTrigger>
        </TabsList>
        
      </div>
      
      <TabsContent value="frp" class="pt-2">
        <FrpTunnel />
      </TabsContent>
      <TabsContent value="cloudflared" class="pt-2">
        <CloudflareTunnel />
      </TabsContent>
    </Tabs>
  </div>
</template>
