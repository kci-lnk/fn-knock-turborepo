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
import OverflowTooltipText from '@admin-shared/components/common/OverflowTooltipText.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../composables/useTargetPolling'
import type { DDNSNetworkInterfacePayload } from '../lib/api'
import { useConfigStore } from '../store/config'

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

type DDNSUpdateScope = 'dual_stack' | 'ipv6_only' | 'ipv4_only'

const UPDATE_SCOPE_KEY = 'update_scope'
const NETWORK_INTERFACE_KEY = 'network_interface'
const NETWORK_INTERFACE_AUTO_VALUE = '__auto__'
const DEFAULT_DDNS_UPDATE_SCOPE: DDNSUpdateScope = 'dual_stack'
const UPDATE_SCOPE_OPTIONS: Array<{ label: string; value: DDNSUpdateScope }> = [
  { label: 'IPv4 & IPv6', value: 'dual_stack' },
  { label: '仅更新 IPv6', value: 'ipv6_only' },
  { label: '仅更新 IPv4', value: 'ipv4_only' },
]

const normalizeUpdateScope = (value: string | null | undefined): DDNSUpdateScope => {
  if (value === 'dual_stack' || value === 'ipv6_only' || value === 'ipv4_only') {
    return value
  }
  return DEFAULT_DDNS_UPDATE_SCOPE
}

const normalizeNetworkInterface = (value: string | null | undefined) => {
  return value?.trim() || ''
}

const toNetworkInterfaceSelectValue = (value: string | null | undefined) => {
  return normalizeNetworkInterface(value) || NETWORK_INTERFACE_AUTO_VALUE
}

const getUpdateScopeLabel = (value: string | null | undefined) => {
  return UPDATE_SCOPE_OPTIONS.find(option => option.value === normalizeUpdateScope(value))?.label || 'IPv4 & IPv6'
}

// ─── State ─────────────────────────────────────────────────────
const isInitialized = ref(false)
const configStore = useConfigStore()
const enabled = ref(true)
const selectedProvider = ref<string>('')
const providers = ref<Provider[]>([])
const providerConfig = ref<Record<string, string>>({})
const lastIP = ref<LastIP>({ ipv4: null, ipv6: null, updated_at: null })
const lastCheck = ref<LastCheck>({ checked_at: null, outcome: null, message: null })
const logs = ref<LogEntry[]>([])
const statusUpdateScope = ref<DDNSUpdateScope>(DEFAULT_DDNS_UPDATE_SCOPE)
const statusNetworkInterface = ref('')
const networkInterfaces = ref<DDNSNetworkInterfacePayload[]>([])

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
const { run: runLoadNetworkInterfaces } = useAsyncAction({
  onError: (error) => {
    console.error('loadNetworkInterfaces:', extractErrorMessage(error, '加载网卡列表失败'))
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
  const def = currentProviderDef.value
  if (!def) return false
  return def.fields.some((field) => providerConfig.value[field.key]?.toString().trim() !== '')
})

const currentUpdateScopeLabel = computed(() => {
  return getUpdateScopeLabel(providerConfig.value[UPDATE_SCOPE_KEY] || statusUpdateScope.value)
})

const selectedNetworkInterface = computed(() => {
  return normalizeNetworkInterface(providerConfig.value[NETWORK_INTERFACE_KEY] || statusNetworkInterface.value)
})

const configuredNetworkInterface = computed(() => {
  return normalizeNetworkInterface(providerConfig.value[NETWORK_INTERFACE_KEY])
})

const resolvedNetworkInterfaces = computed(() => {
  const items = [...networkInterfaces.value]
  const selected = selectedNetworkInterface.value
  if (selected && !items.some(item => item.name === selected)) {
    items.push({
      name: selected,
      label: `${selected}（当前配置，暂不可用）`,
      summary: '当前配置中的网卡已不可用或没有可用地址',
      hasIpv4: false,
      hasIpv6: false,
      addresses: [],
    })
  }
  return items
})

const currentNetworkInterfaceLabel = computed(() => {
  const selected = selectedNetworkInterface.value
  if (!selected) {
    return '自动选择'
  }
  return resolvedNetworkInterfaces.value.find(item => item.name === selected)?.label || selected
})

const configuredNetworkInterfaceLabel = computed(() => {
  const selected = configuredNetworkInterface.value
  if (!selected) {
    return '自动选择'
  }
  return resolvedNetworkInterfaces.value.find(item => item.name === selected)?.label || selected
})

const selectedNetworkInterfaceDetail = computed(() => {
  return configuredNetworkInterface.value ? configuredNetworkInterfaceLabel.value : ''
})

const effectiveUpdateScope = computed<DDNSUpdateScope>(() => {
  return normalizeUpdateScope(providerConfig.value[UPDATE_SCOPE_KEY] || statusUpdateScope.value)
})

const showIPv4Status = computed(() => effectiveUpdateScope.value !== 'ipv6_only')
const showIPv6Status = computed(() => effectiveUpdateScope.value !== 'ipv4_only')
const isEnabledSwitchDisabled = computed(() => isTogglingEnabled.value || isLoading.value)
const isProviderSelectDisabled = computed(() => isSwitchingProvider.value || isLoading.value)
const isSubdomainMode = computed(() => configStore.config?.run_type === 3)

const getFieldDescription = (field: ProviderField) => {
  const description = field.description?.trim() || ''

  if (isSubdomainMode.value && field.key === 'domain' && field.label === '完整域名') {
    const wildcardHint = '子域模式下可填写如 *.example.com，使用星号可设置泛解析。'
    return description ? `${description} ${wildcardHint}` : wildcardHint
  }

  return description
}

async function loadStatus() {
  await runLoadStatus(async () => {
    const status = await DDNSAPI.getStatus()
    enabled.value = status.enabled
    if (status.provider) {
      selectedProvider.value = status.provider
    }
    lastIP.value = status.lastIP
    lastCheck.value = status.lastCheck
    statusUpdateScope.value = normalizeUpdateScope(status.updateScope)
    statusNetworkInterface.value = normalizeNetworkInterface(status.networkInterface)
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

async function loadNetworkInterfaces() {
  await runLoadNetworkInterfaces(async () => {
    networkInterfaces.value = await DDNSAPI.getNetworkInterfaces()
  })
}

async function loadConfig() {
  if (!selectedProvider.value) return
  await runLoadConfig(async () => {
    const config = await DDNSAPI.getConfig(selectedProvider.value)
    const def = currentProviderDef.value
    const merged: Record<string, string> = {
      [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config[UPDATE_SCOPE_KEY]),
      [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(config[NETWORK_INTERFACE_KEY]),
    }

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
    statusUpdateScope.value = normalizeUpdateScope(status.updateScope)
    statusNetworkInterface.value = normalizeNetworkInterface(status.networkInterface)
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

async function copyTextToClipboard(text: string) {
  if (typeof navigator !== 'undefined' && navigator.clipboard?.writeText) {
    await navigator.clipboard.writeText(text)
    return
  }

  if (typeof document === 'undefined') {
    throw new Error('Clipboard API unavailable')
  }

  const textarea = document.createElement('textarea')
  textarea.value = text
  textarea.setAttribute('readonly', '')
  textarea.style.position = 'fixed'
  textarea.style.top = '0'
  textarea.style.left = '0'
  textarea.style.opacity = '0'

  document.body.appendChild(textarea)
  textarea.focus()
  textarea.select()
  textarea.setSelectionRange(0, textarea.value.length)

  const copied = document.execCommand('copy')
  document.body.removeChild(textarea)

  if (!copied) {
    throw new Error('execCommand copy failed')
  }
}

async function copyIpAddress(versionLabel: 'IPv4' | 'IPv6', value: string | null) {
  const address = value?.trim()
  if (!address) {
    toast.error(`${versionLabel} 地址不可用`)
    return
  }

  try {
    await copyTextToClipboard(address)
    toast.success(`${versionLabel} 地址已复制`, { description: address })
  }
  catch (error) {
    console.error('copyIpAddress:', error)
    toast.error(`复制 ${versionLabel} 地址失败`, {
      description: '当前页面可能运行在受限 iframe 中，请手动复制。',
    })
  }
}

const logLines = computed(() =>
  logs.value.map((e) => {
    const tag = e.level === 'error' ? '[错误]' : e.level === 'warn' ? '[警告]' : '[信息]'
    return `${tag} ${formatTime(e.time)}  ${e.message}`
  }),
)

onMounted(async () => {
  const initialized = await runInitialize(async () => {
    await Promise.all([loadProviders(), loadStatus(), loadNetworkInterfaces()])
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
        <Switch v-model="enabled" :disabled="isEnabledSwitchDisabled" />
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
        <div class="flex flex-col gap-4 md:flex-row md:items-center md:justify-between md:gap-6">
          <div class="flex min-w-0 flex-col gap-4 md:flex-1 md:flex-row md:items-center md:gap-6">
            <div v-if="showIPv4Status" class="flex items-center gap-4 shrink-0">
              <div class="p-2.5 rounded-xl">
                <Wifi class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">IPv4地址</p>
                <button
                  type="button"
                  class="block text-left text-sm font-mono font-medium transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 rounded-sm disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!lastIP.ipv4"
                  @click="copyIpAddress('IPv4', lastIP.ipv4)"
                >
                  {{ lastIP.ipv4 || '---.---.---.---' }}
                </button>
              </div>
            </div>

            <div
              v-if="showIPv6Status"
              class="flex min-w-0 items-center gap-4"
              :class="showIPv4Status ? 'md:border-l md:pl-6' : ''">
              <div class="p-2.5 rounded-xl shrink-0">
                <Globe class="h-5 w-5" />
              </div>
              <div class="space-y-1 overflow-hidden w-full">
                <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">IPv6地址</p>
                <button
                  type="button"
                  class="block min-w-0 max-w-full rounded-sm text-left transition-colors hover:text-primary focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:text-foreground"
                  :disabled="!lastIP.ipv6"
                  @click="copyIpAddress('IPv6', lastIP.ipv6)"
                >
                  <OverflowTooltipText
                    as="span"
                    :text="lastIP.ipv6 || '未检测到地址'"
                    class="text-sm font-mono font-medium"
                  />
                </button>
              </div>
            </div>
          </div>

          <div class="flex flex-wrap items-center gap-4 md:ml-auto md:flex-nowrap md:gap-6 md:border-l md:pl-6">
            <div class="flex items-center gap-4 shrink-0">
              <div class="p-2.5 rounded-xl">
                <RefreshCw class="h-5 w-5" :class="{ 'animate-spin': isTesting }" />
              </div>
              <div class="space-y-1">
                <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">最后成功更新</p>
                <p class="text-sm font-medium"><HumanFriendlyTime :value="lastIP.updated_at" empty-text="从未" /></p>
              </div>
            </div>

            <div class="flex items-center gap-4 shrink-0">
              <div class="p-2.5 rounded-xl">
                <Globe class="h-5 w-5" />
              </div>
              <div class="space-y-1">
                <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">更新范围</p>
                <p class="text-sm font-medium">{{ currentUpdateScopeLabel }}</p>
              </div>
            </div>

            <div class="flex items-center gap-4 shrink-0">
              <div class="p-2.5 rounded-xl">
                <Wifi class="h-5 w-5" />
              </div>
              <div class="space-y-1 max-w-[240px]">
                <p class="text-[10px] uppercase tracking-wider font-semibold text-muted-foreground">出站网卡</p>
                <OverflowTooltipText
                  as="p"
                  :text="currentNetworkInterfaceLabel"
                  class="text-sm font-medium"
                />
              </div>
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
                :disabled="isProviderSelectDisabled"
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

          <div v-if="selectedProvider"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-network-interface" class="text-sm font-medium">出站网卡</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                测试更新和自动更新都会优先从这里选择的网卡发起请求
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="toNetworkInterfaceSelectValue(providerConfig[NETWORK_INTERFACE_KEY])"
                @update:modelValue="(val: any) => providerConfig[NETWORK_INTERFACE_KEY] = val === NETWORK_INTERFACE_AUTO_VALUE ? '' : String(val ?? '')">
                <SelectTrigger
                  class="w-full overflow-hidden"
                  id="ddns-network-interface">
                  <SelectValue :placeholder="'自动选择'">
                    <span class="block min-w-0 max-w-full truncate">
                      {{ configuredNetworkInterfaceLabel }}
                    </span>
                  </SelectValue>
                </SelectTrigger>
                <SelectContent class="w-[var(--reka-select-trigger-width)] max-w-[min(32rem,calc(100vw-2rem))]">
                  <SelectItem :value="NETWORK_INTERFACE_AUTO_VALUE">
                    自动选择
                  </SelectItem>
                  <SelectItem
                    v-for="networkInterface in resolvedNetworkInterfaces"
                    :key="networkInterface.name"
                    :value="networkInterface.name">
                    <div class="min-w-0 flex-1 pr-5">
                      <OverflowTooltipText
                        :text="networkInterface.label"
                        class="text-sm"
                        tooltip-align="start"
                        tooltip-side="right"
                      />
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>

              <p
                v-if="selectedNetworkInterfaceDetail"
                class="text-[11px] leading-5 text-muted-foreground break-all"
              >
                {{ selectedNetworkInterfaceDetail }}
              </p>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                测试更新和自动更新都会优先从这里选择的网卡发起请求
              </p>
            </div>
          </div>

          <div v-if="selectedProvider"
            class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
            <div class="space-y-1 mt-1.5">
              <Label for="ddns-update-scope" class="text-sm font-medium">更新范围</Label>
              <p class="text-xs text-muted-foreground hidden sm:block pr-4">
                更新 IPv4、IPv6，或同时更新两者
              </p>
            </div>
            <div class="w-full max-w-md space-y-2">
              <Select
                :modelValue="providerConfig[UPDATE_SCOPE_KEY] || DEFAULT_DDNS_UPDATE_SCOPE"
                @update:modelValue="(val: any) => providerConfig[UPDATE_SCOPE_KEY] = normalizeUpdateScope(String(val ?? ''))">
                <SelectTrigger class="w-full" id="ddns-update-scope">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="option in UPDATE_SCOPE_OPTIONS" :key="option.value" :value="option.value">
                    {{ option.label }}
                  </SelectItem>
                </SelectContent>
              </Select>

              <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                更新 IPv4、IPv6，或同时更新两者
              </p>
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
                <p v-if="getFieldDescription(field)" class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                  {{ getFieldDescription(field) }}
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

                <p v-if="getFieldDescription(field)" class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                  {{ getFieldDescription(field) }}
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
