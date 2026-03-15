<script setup lang="ts">
import { ref, watch, onMounted, onUnmounted, computed } from 'vue'
import { DDNSAPI } from '../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Switch } from '@/components/ui/switch'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { RefreshCw, Trash2, Globe, Wifi, Clock, Eye, EyeOff } from 'lucide-vue-next'
import LiveStatusBadge from '@/components/LiveStatusBadge.vue'
import { toast } from '@admin-shared/utils/toast'
import LogViewer from '@admin-shared/components/LogViewer.vue'
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue'
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../composables/useTargetPolling'

interface ProviderField {
  key: string
  label: string
  type: 'text' | 'password' | 'select'
  placeholder?: string
  required?: boolean
  options?: { label: string; value: string }[]
  description?: string
}

interface Provider {
  name: string
  label: string
  fields: ProviderField[]
}

interface LogEntry {
  time: string
  level: 'info' | 'error' | 'warn'
  message: string
}

interface LastIP {
  ipv4: string | null
  ipv6: string | null
  updated_at: string | null
}

interface LastCheck {
  checked_at: string | null
  outcome: 'updated' | 'noop' | 'skipped' | 'error' | null
  message: string | null
}

// ─── State ─────────────────────────────────────────────────────
const isInitialized = ref(false)
const enabled = ref(true)
const selectedProvider = ref<string>('')
const providers = ref<Provider[]>([])
const providerConfig = ref<Record<string, string>>({})
const lastIP = ref<LastIP>({ ipv4: null, ipv6: null, updated_at: null })
const lastCheck = ref<LastCheck>({ checked_at: null, outcome: null, message: null })
const logs = ref<LogEntry[]>([])

const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  rethrow: true,
  onError: (error) => {
    toast.error('保存配置失败', { description: extractErrorMessage(error, '保存配置失败') })
  },
})
const { isPending: isTesting, run: runTestUpdate } = useAsyncAction({
  onError: (error) => {
    toast.error('更新失败', { description: extractErrorMessage(error, '更新失败') })
  },
})
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: () => {
    toast.error('清空日志失败')
  },
})
const { isPending: isTogglingEnabled, run: runToggleEnabled } = useAsyncAction({
  onError: (error) => {
    toast.error('切换失败', { description: extractErrorMessage(error, '切换失败') })
  },
})
const { isPending: isSwitchingProvider, run: runSwitchProvider } = useAsyncAction({
  onError: (error) => {
    toast.error('切换提供商失败', { description: extractErrorMessage(error, '切换提供商失败') })
  },
})
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    console.error('loadStatus:', extractErrorMessage(error, '加载状态失败'))
  },
})
const { run: runLoadProviders } = useAsyncAction({
  onError: (error) => {
    console.error('loadProviders:', extractErrorMessage(error, '加载提供商失败'))
  },
})
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    console.error('loadConfig:', extractErrorMessage(error, '加载配置失败'))
  },
})
const { isPending: isLoading, run: runInitialize } = useAsyncAction({
  onError: (error) => {
    toast.error('初始化失败', { description: extractErrorMessage(error, '加载 DDNS 管理页面失败') })
  },
})

const fieldVisibility = ref<Record<string, boolean>>({})
const fieldEditReady = ref<Record<string, boolean>>({})

const toggleFieldVisibility = (key: string) => {
  fieldVisibility.value[key] = !fieldVisibility.value[key]
}

const getFieldStateKey = (key: string) => `${selectedProvider.value}:${key}`

const getFieldDomId = (index: number) => `ddns-field-${index}`

const getFieldInputName = (index: number) => `ddns-input-${index}`

const enableFieldEditing = (key: string) => {
  fieldEditReady.value[getFieldStateKey(key)] = true
}

const isFieldEditReady = (key: string) => {
  return fieldEditReady.value[getFieldStateKey(key)] === true
}

const getFieldAutocomplete = (field: ProviderField) => {
  const normalizedKey = field.key.toLowerCase()
  if (
    field.type === 'password'
    || /access|account|auth|credential|email|key|login|secret|token|user/.test(normalizedKey)
  ) {
    return 'new-password'
  }

  return 'off'
}

const currentProviderDef = computed(() => {
  return providers.value.find(p => p.name === selectedProvider.value) || null
})

const hasProviderConfig = computed(() => {
  return Object.values(providerConfig.value).some((value) => value?.toString().trim() !== '')
})

async function loadStatus() {
  await runLoadStatus(async () => {
    const status = await DDNSAPI.getStatus()
    enabled.value = status.enabled
    if (status.provider) {
      selectedProvider.value = status.provider
    }
    lastIP.value = status.lastIP
    lastCheck.value = status.lastCheck
  })
}

async function loadProviders() {
  await runLoadProviders(async () => {
    const data = await DDNSAPI.getProviders()
    providers.value = data.map(p => ({
      ...p,
      fields: p.fields.map(f => ({ ...f, type: f.type as 'text' | 'password' | 'select' }))
    }))
  })
}

async function loadConfig() {
  if (!selectedProvider.value) return
  await runLoadConfig(async () => {
    const config = await DDNSAPI.getConfig(selectedProvider.value)
    const def = currentProviderDef.value
    const merged: Record<string, string> = {}

    fieldEditReady.value = {}

    if (def) {
      for (const f of def.fields) {
        const val = config[f.key] ?? ''
        merged[f.key] = val
        if (f.type === 'password' && !(f.key in fieldVisibility.value)) {
          fieldVisibility.value[f.key] = true
        }
      }
    }
    providerConfig.value = merged
  })
}

const ddnsPolling = useTargetPolling({
  target: 'ddns',
  intervalMs: 2000,
  onData: (payload) => {
    logs.value = mergePollingLogWindow(logs.value, payload.logs as LogEntry[], {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    })

    const status = payload.status
    lastIP.value = status.lastIP
    lastCheck.value = status.lastCheck
    if (status.provider) {
      selectedProvider.value = status.provider
    }
    if (enabledInitialized && status.enabled !== enabled.value) {
      enabledInitialized = false
      enabled.value = status.enabled
      enabledInitialized = true
    }
  },
  onError: (error) => {
    console.error('ddns poll:', extractErrorMessage(error, '轮询 DDNS 状态失败'))
  },
})

let enabledInitialized = false
watch(enabled, async (val) => {
  if (!enabledInitialized) return
  await runToggleEnabled(
    () => DDNSAPI.toggle(val),
    {
      onSuccess: () => {
        toast.success(val ? '已开启自动更新' : '已关闭自动更新')
      },
      onError: () => {
        enabledInitialized = false
        enabled.value = !val
        enabledInitialized = true
      },
    },
  )
})

async function onProviderChange(val: string) {
  if (!val || val === selectedProvider.value) return
  await runSwitchProvider(async () => {
    await DDNSAPI.setProvider(val)
    selectedProvider.value = val
    await loadConfig()
  })
}

async function onSaveConfigSilent() {
  if (!selectedProvider.value) return false
  await runSaveConfig(() => DDNSAPI.saveConfig(selectedProvider.value, providerConfig.value))
  return true
}

async function onTest() {
  await runTestUpdate(async () => {
    await onSaveConfigSilent()
    const result = await DDNSAPI.test()
    if (result.success) {
      toast.success('更新成功')
      await loadStatus()
      return
    }
    toast.error('更新失败', { description: result.message })
  })
}

async function onClearLogs() {
  await runClearLogs(
    () => DDNSAPI.clearLogs(),
    {
      onSuccess: () => {
        logs.value = []
        ddnsPolling.resetCursor()
        void ddnsPolling.refresh()
        toast.success('日志已清空')
      },
    },
  )
}

function formatTime(iso: string | null): string {
  return formatDateTimeSafe(iso, { locale: 'zh-CN', emptyText: '从未' })
}

const logLines = computed(() =>
  logs.value.map((e) => {
    const tag = e.level === 'error' ? '[错误]' : e.level === 'warn' ? '[警告]' : '[信息]'
    return `${tag} ${formatTime(e.time)}  ${e.message}`
  }),
)

onMounted(async () => {
  const initialized = await runInitialize(async () => {
    await Promise.all([loadProviders(), loadStatus()])
    enabledInitialized = true
    await loadConfig()
    return true
  })
  isInitialized.value = true
  if (initialized) {
    ddnsPolling.start()
  }
})
onUnmounted(() => {
  ddnsPolling.stop()
})
</script>

<template>
  <div v-if="isInitialized && !isLoading" class="space-y-3">
    <div class="flex items-center justify-between">
      <h2 class="text-xl font-semibold">DDNS 管理</h2>
      <div class="flex items-center gap-3">
        <span class="text-sm text-muted-foreground">{{ enabled ? '已开启自动更新' : '已关闭自动更新' }}</span>
        <Switch v-model="enabled" :disabled="isTogglingEnabled || isLoading" />
      </div>
    </div>

    <Card class="overflow-hidden py-5 mb-6">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="text-base font-medium flex items-center gap-2">
            运行状态
            <LiveStatusBadge
              :active="enabled"
              active-label="已启用"
              inactive-label="已暂停"
              class="mt-px"
            />
          </CardTitle>

          <div v-if="enabled"
            class="flex items-center gap-1.5 text-xs text-muted-foreground bg-muted/50 px-2 py-1 rounded-md">
            <Clock class="h-3.5 w-3.5" />
            <span>每 5 分钟自动同步</span>
          </div>
        </div>
      </CardHeader>

      <CardContent>
        <div class="flex flex-col md:flex-row gap-4 md:gap-6 items-start md:items-center">

          <div class="flex items-center gap-4 shrink-0">
            <div class="p-2.5 rounded-xl">
              <Wifi class="h-5 w-5" />
            </div>
            <div class="space-y-1">
              <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">IPv4地址</p>
              <p class="text-sm font-mono font-medium">{{ lastIP.ipv4 || '---.---.---.---' }}</p>
            </div>
          </div>

          <div class="flex items-center gap-4 flex-1 md:border-x md:px-6 min-w-0">
            <div class="p-2.5 rounded-xl shrink-0">
              <Globe class="h-5 w-5" />
            </div>
            <div class="space-y-1 overflow-hidden w-full">
              <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">IPv6地址</p>
              <p class="text-sm font-mono font-medium truncate" :title="lastIP.ipv6 || ''">
                {{ lastIP.ipv6 || '未检测到地址' }}
              </p>
            </div>
          </div>

          <div class="flex items-center gap-4 shrink-0">
            <div class="p-2.5 rounded-xl">
              <RefreshCw class="h-5 w-5" :class="{ 'animate-spin': isTesting }" />
            </div>
            <div class="space-y-1">
              <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">最后成功更新</p>
              <p class="text-sm font-medium"><HumanFriendlyTime :value="lastIP.updated_at" empty-text="从未" /></p>
            </div>
          </div>

        </div>

      </CardContent>
    </Card>

    <ConfigCollapsibleCard title="提供商配置" :configured="hasProviderConfig" :ready="!isLoading"
      expanded-content-class="p-0 sm:p-0">
      <template #summary>
        当前提供商: {{ currentProviderDef?.label || '未配置' }}
      </template>

      <template #default>
        <div class="divide-y divide-border">
          <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start">
            <div class="space-y-1 mt-1.5">
              <Label class="text-sm font-medium">DDNS 提供商</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">选择您要用于解析的 DNS 服务商</p>
            </div>
            <div class="w-full max-w-md">
              <Select :modelValue="selectedProvider"
                :disabled="isSwitchingProvider || isLoading"
                @update:modelValue="(val: any) => onProviderChange(String(val ?? ''))">
                <SelectTrigger class="w-full" id="ddns-provider">
                  <SelectValue placeholder="选择提供商" />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="p in providers" :key="p.name" :value="p.name">
                    {{ p.label }}
                  </SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>

          <template v-if="currentProviderDef">
            <div v-for="(field, index) in currentProviderDef.fields" :key="field.key"
              class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">

              <div class="space-y-1 mt-1.5">
                <Label :for="getFieldDomId(index)" class="text-sm font-medium flex items-center gap-1">
                  {{ field.label }}
                  <span v-if="field.required !== false" class="text-destructive">*</span>
                </Label>
                <p v-if="field.description" class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                  {{ field.description }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <Select v-if="field.type === 'select' && field.options"
                  :modelValue="providerConfig[field.key] || (field.options && field.options[0]?.value) || ''"
                  @update:modelValue="(val: any) => providerConfig[field.key] = String(val ?? '')">
                  <SelectTrigger class="w-full" :id="getFieldDomId(index)">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem v-for="opt in field.options" :key="opt.value" :value="opt.value">
                      {{ opt.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>

                <div v-else-if="field.type === 'password'" class="relative">
                  <Input :id="getFieldDomId(index)" :name="getFieldInputName(index)"
                    :type="fieldVisibility[field.key] ? 'text' : 'password'" :placeholder="field.placeholder"
                    :autocomplete="getFieldAutocomplete(field)" :readonly="!isFieldEditReady(field.key)"
                    v-model="providerConfig[field.key]" class="pr-10" @focus="enableFieldEditing(field.key)"
                    @pointerdown="enableFieldEditing(field.key)" />
                  <button type="button"
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    @click="toggleFieldVisibility(field.key)">
                    <component :is="fieldVisibility[field.key] ? EyeOff : Eye" class="h-4 w-4" />
                  </button>
                </div>

                <Input v-else :id="getFieldDomId(index)" :name="getFieldInputName(index)" :type="field.type"
                  :placeholder="field.placeholder" :autocomplete="getFieldAutocomplete(field)"
                  :readonly="!isFieldEditReady(field.key)" v-model="providerConfig[field.key]"
                  @focus="enableFieldEditing(field.key)" @pointerdown="enableFieldEditing(field.key)" />

                <p v-if="field.description" class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                  {{ field.description }}
                </p>
              </div>
            </div>
          </template>

        </div>
      </template>

      <template #actions="{ collapse }">
        <div class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg">
          <Button variant="outline" @click="collapse">折叠</Button>
          <Button :disabled="isTesting || isSaving || !selectedProvider" @click="onTest" class="min-w-[100px] shadow-sm">
            <RefreshCw v-if="isTesting" class="w-4 h-4 mr-2 animate-spin" />
            {{ isTesting ? '更新中...' : '保存并更新' }}
          </Button>
        </div>
      </template>
    </ConfigCollapsibleCard>

    <Card class="gap-2">
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle class="text-base">日志</CardTitle>
          <div class="flex gap-2">
            <Button variant="outline" size="sm" :disabled="isClearingLogs || logs.length === 0" @click="onClearLogs">
              <Trash2 class="h-3.5 w-3.5 mr-1" />
              清空
            </Button>
          </div>
        </div>
      </CardHeader>
      <CardContent>
        <LogViewer :logs="logLines" reversed height-class="max-h-[400px]" :show-header="false" theme="light" wrap />
      </CardContent>
    </Card>
  </div>

  <div v-else class="flex h-full items-center justify-center min-h-[400px]">
    <div class="animate-spin rounded-full h-8 w-8 border-b-2 border-primary"></div>
  </div>
</template>
