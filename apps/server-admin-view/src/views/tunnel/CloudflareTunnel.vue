<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { CloudflaredAPI, SystemAPI, ConfigAPI, type CloudflaredProtocol } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { EyeIcon, EyeOffIcon, TriangleAlert, Trash2 } from 'lucide-vue-next'
import { toast } from '@admin-shared/utils/toast'
import LogViewer from '@admin-shared/components/LogViewer.vue'
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../../composables/useTargetPolling'
import { useConfigStore } from '../../store/config'

type CloudflaredLogAnalysis = {
  reason: 'origin_tls_hostname_mismatch'
  requestedHost: string
  certificateHosts: string[]
  originUrl?: string
  originHost?: string
  evidence: string
}

const ORIGIN_TLS_HOSTNAME_MISMATCH_REGEX =
  /tls:\s*failed to verify certificate:\s*x509:\s*certificate is valid for\s+(.+),\s*not\s+([^\s"]+)/i
const DESTINATION_URL_REGEX = /\bdest=(https?:\/\/[^\s"]+)/i
const { t } = useI18n()

type CloudflaredProtocolOption = {
  value: CloudflaredProtocol
  label: string
  description: string
}

const cloudflaredProtocolOptions = computed<CloudflaredProtocolOption[]>(() => [
  {
    value: 'auto',
    label: t('admin.cloudflareTunnel.protocol.auto'),
    description: t('admin.cloudflareTunnel.protocol.autoDescription'),
  },
  {
    value: 'http2',
    label: 'HTTP2',
    description: t('admin.cloudflareTunnel.protocol.http2Description'),
  },
  {
    value: 'quic',
    label: 'QUIC',
    description: t('admin.cloudflareTunnel.protocol.quicDescription'),
  },
])

const defaultCloudflaredProtocolOption = computed<CloudflaredProtocolOption>(
  () =>
    cloudflaredProtocolOptions.value[0] ?? {
      value: 'auto',
      label: t('admin.cloudflareTunnel.protocol.auto'),
      description: t('admin.cloudflareTunnel.protocol.autoDescription'),
    },
)

const router = useRouter()
const configStore = useConfigStore()

const isInit = ref<boolean>(false)
const running = ref<boolean>(false)
const pid = ref<number | null>(null)
const logs = ref<string[]>([])
const cloudflaredLogAnalysis = ref<CloudflaredLogAnalysis | null>(null)
const showInitDialog = ref(false)
const showToken = ref(true)
const configLoaded = ref(false)
const hasCloudflaredLogBaseline = ref(false)
const accessEntryPort = ref('7999')

const token = ref<string>('')
const protocol = ref<CloudflaredProtocol>('auto')
const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.cloudflareTunnel.saveFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.saveFailed')) })
  },
})
const { isPending: isStarting, run: runStartCloudflared } = useAsyncAction()
const { isPending: isStopping, run: runStopCloudflared } = useAsyncAction()
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.cloudflareTunnel.clearLogsFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.clearLogsFailed')) })
  },
})
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.cloudflareTunnel.loadStatusFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.loadStatusFailed')) })
  },
})
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.cloudflareTunnel.loadConfigFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.loadConfigFailed')) })
  },
})

watch(token, (newVal) => {
  if (!newVal) return
  // Token normally starts with eyJ and is a base64 encoded JSON string over ~100 characters long
  const rawTokenMatch = newVal.match(/(eyJ[A-Za-z0-9-_]+)/)
  if (rawTokenMatch && rawTokenMatch[1]) {
    const extracted = rawTokenMatch[1]
    if (newVal !== extracted) {
      token.value = extracted
      toast.success(t('admin.cloudflareTunnel.tokenExtracted'))
    }
  }
})

const canStart = computed(() => isInit.value && !running.value && token.value)
const canStop = computed(() => running.value)
const isReverseProxySubdomainMode = computed(
  () =>
    configStore.config?.run_type === 1 &&
    configStore.config?.reverse_proxy_submode === 'subdomain',
)
const rootDomain = computed(
  () => configStore.config?.subdomain_mode?.root_domain?.trim().toLowerCase() || '',
)
const publicWildcardHostname = computed(() =>
  rootDomain.value ? `*.${rootDomain.value}` : '*.example.com',
)
const authServiceHost = computed(() => {
  const authMapping = configStore.config?.host_mappings?.find(
    (mapping) => mapping.service_role === 'auth',
  )
  return (
    authMapping?.host?.trim() ||
    configStore.config?.subdomain_mode?.auth_host?.trim() ||
    ''
  )
})
const displayAccessEntryPort = computed(() => accessEntryPort.value.trim() || '7999')
const cloudflaredOriginServiceUrl = computed(
  () => `http://127.0.0.1:${displayAccessEntryPort.value}`,
)
const hasSubdomainRoot = computed(() => Boolean(rootDomain.value))
const cloudflaredProtocolOption = computed(
  () =>
    cloudflaredProtocolOptions.value.find((option) => option.value === protocol.value) ??
    defaultCloudflaredProtocolOption.value,
)
const cloudflaredProtocolLabel = computed(() => cloudflaredProtocolOption.value.label)
const cloudflaredProtocolDescription = computed(
  () => cloudflaredProtocolOption.value.description,
)
const cloudflaredLogAnalysisMessage = computed(() => {
  const analysis = cloudflaredLogAnalysis.value
  if (!analysis) return ''

  const certificateTargets = analysis.certificateHosts.join(', ')
  const originTarget = analysis.originHost
    ? t('admin.cloudflareTunnel.analysisOriginHost', { host: analysis.originHost })
    : t('admin.cloudflareTunnel.analysisOriginGeneric')

  return t('admin.cloudflareTunnel.analysisMessage', {
    origin: originTarget,
    certificates: certificateTargets,
    requested: analysis.requestedHost,
  })
})

function analyzeCloudflaredLogs(lines: string[]): CloudflaredLogAnalysis | null {
  for (let index = lines.length - 1; index >= 0; index -= 1) {
    const line = lines[index]?.trim()
    if (!line) continue

    const mismatchMatch = line.match(ORIGIN_TLS_HOSTNAME_MISMATCH_REGEX)
    if (!mismatchMatch) continue

    const certificateHosts = mismatchMatch[1]
      ?.split(',')
      .map((item) => item.trim())
      .filter(Boolean) ?? []
    const requestedHost = mismatchMatch[2]?.trim()
    if (!certificateHosts.length || !requestedHost) continue

    const originUrl = line.match(DESTINATION_URL_REGEX)?.[1]
    let originHost: string | undefined
    if (originUrl) {
      try {
        originHost = new URL(originUrl).hostname
      } catch {
        originHost = undefined
      }
    }

    return {
      reason: 'origin_tls_hostname_mismatch',
      requestedHost,
      certificateHosts,
      originUrl,
      originHost,
      evidence: line,
    }
  }

  return null
}

async function loadStatus() {
  await runLoadStatus(async () => {
    const st = await CloudflaredAPI.getStatus()
    isInit.value = st.initialized
    running.value = st.running
    pid.value = st.pid
    if (!isInit.value) {
      const sys = await SystemAPI.getCloudflaredStatus()
      if (!sys?.data?.downloaded) {
        showInitDialog.value = true
      }
    }
  })
}

async function loadConfig() {
  await runLoadConfig(
    async () => {
      const res = await CloudflaredAPI.getConfig()
      token.value = res.token || ''
      protocol.value = res.protocol || 'auto'
    },
    {
      onFinally: () => {
        configLoaded.value = true
      },
    },
  )
}

async function loadAccessEntryPort() {
  try {
    const info = await SystemAPI.getAccessEntry()
    accessEntryPort.value = info.port.trim() || '7999'
  } catch (error) {
    console.warn('load cloudflared access entry port failed:', error)
  }
}

async function saveConfig() {
  await runSaveConfig(async () => {
    await CloudflaredAPI.saveConfig({
      token: token.value.trim(),
      protocol: protocol.value,
    })
    const shouldRestart = running.value
    if (shouldRestart) {
      await stopCloudflared({ silent: true })
      await startCloudflared({ silent: true })
      toast.success(t('admin.cloudflareTunnel.restartSuccess'))
      return
    }
    toast.success(t('admin.cloudflareTunnel.saveSuccess'))
  })
}

async function startCloudflared(options?: { silent?: boolean }) {
  await runStartCloudflared(
    () => CloudflaredAPI.start(),
    {
      onSuccess: async (res) => {
        pid.value = res.pid
        running.value = true
        await ConfigAPI.updateDefaultTunnel('cloudflared')
        if (configStore.config) {
          configStore.config.default_tunnel = 'cloudflared'
        }
        if (!options?.silent) toast.success(t('admin.cloudflareTunnel.startSuccess'))
      },
      onError: (error) => {
        if (options?.silent) return
        toast.error(t('admin.cloudflareTunnel.startFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.startFailed')) })
      },
    },
  )
}

async function stopCloudflared(options?: { silent?: boolean }) {
  await runStopCloudflared(
    () => CloudflaredAPI.stop(),
    {
      onSuccess: () => {
        running.value = false
        pid.value = null
        if (!options?.silent) toast.success(t('admin.cloudflareTunnel.stopSuccess'))
      },
      onError: (error) => {
        if (options?.silent) return
        toast.error(t('admin.cloudflareTunnel.stopFailed'), { description: extractErrorMessage(error, t('admin.cloudflareTunnel.stopFailed')) })
      },
    },
  )
}

async function onClearLogsClick() {
  await runClearLogs(
    () => CloudflaredAPI.clearLogs(),
    {
      onSuccess: () => {
        logs.value = []
        cloudflaredLogAnalysis.value = null
        cloudflaredPolling.resetCursor()
        void cloudflaredPolling.refresh()
        toast.success(t('admin.cloudflareTunnel.logsCleared'))
      },
    },
  )
}

function gotoResources() {
  showInitDialog.value = false
  router.push({ path: '/system', query: { tab: 'cloudflared' } })
}

const cloudflaredPolling = useTargetPolling({
  target: 'cloudflared',
  intervalMs: 2000,
  onData: (payload) => {
    logs.value = mergePollingLogWindow(logs.value, payload.logs, {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    })

    running.value = payload.status.running
    pid.value = payload.status.pid

    if (!hasCloudflaredLogBaseline.value) {
      hasCloudflaredLogBaseline.value = true
      return
    }

    const nextAnalysis = analyzeCloudflaredLogs(payload.logs)
    if (nextAnalysis) {
      cloudflaredLogAnalysis.value = nextAnalysis
    }
  },
})

onMounted(async () => {
  await Promise.all([
    loadStatus(),
    loadConfig(),
    loadAccessEntryPort(),
    configStore.config ? Promise.resolve() : configStore.loadConfig(),
  ])
  cloudflaredPolling.start()
})
onUnmounted(() => {
  cloudflaredPolling.stop()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <h2 class="text-xl font-semibold">{{ t('admin.cloudflareTunnel.title') }}</h2>
      <div class="flex gap-2">
        <Button v-if="!running" :disabled="!canStart || isStarting" @click="startCloudflared">{{ t('admin.cloudflareTunnel.start') }}</Button>
        <Button v-else variant="destructive" :disabled="!canStop || isStopping" @click="stopCloudflared">{{ t('admin.cloudflareTunnel.stop') }}</Button>
      </div>
    </div>

    <div class="grid grid-cols-1">
      <ConfigCollapsibleCard
        :title="t('admin.cloudflareTunnel.configTitle')"
        :configured="Boolean(token)"
        :ready="configLoaded"
        expanded-content-class="p-0 sm:p-0"
      >
        <template #summary>
          {{ t('admin.cloudflareTunnel.configSummary', { token: token ? '********' : t('admin.cloudflareTunnel.notConfigured'), protocol: cloudflaredProtocolLabel }) }}
        </template>

        <template #default>
          <div class="divide-y divide-border">
            <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
              <div class="space-y-1 mt-1.5">
                <Label for="cloudflared-token" class="text-sm font-medium flex items-center gap-1">
                  Tunnel Token
                  <span class="text-destructive">*</span>
                </Label>
                <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                  {{ t('admin.cloudflareTunnel.tokenDescription') }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <div class="relative">
                  <Input id="cloudflared-token" v-model.trim="token" class="pr-10" placeholder="eyJh..."
                    :type="showToken ? 'text' : 'password'" :autocomplete="showToken ? 'off' : 'new-password'"
                    autocapitalize="off" autocorrect="off" :spellcheck="false" data-form-type="other"
                    data-1p-ignore="true" data-lpignore="true" data-bwignore="true" />
                  <button
                    type="button"
                    class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
                    @click="showToken = !showToken"
                  >
                    <EyeIcon v-if="showToken" class="w-4 h-4" />
                    <EyeOffIcon v-else class="w-4 h-4" />
                  </button>
                </div>
                <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                  {{ t('admin.cloudflareTunnel.tokenDescription') }}
                </p>
                <div class="text-xs text-muted-foreground mt-2 space-y-1 leading-relaxed">
                  <p>{{ t('admin.cloudflareTunnel.configSourcePrefix') }} <a href="https://one.dash.cloudflare.com/" target="_blank" class="text-primary hover:underline font-medium">Cloudflare Zero Trust Dashboard</a></p>
                  <p>{{ t('admin.cloudflareTunnel.createTunnelHint') }}</p>
                  <p>{{ t('admin.cloudflareTunnel.copyTokenHint') }}</p>
                </div>
              </div>
            </div>
            <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
              <div class="space-y-1 mt-1.5">
                <Label for="cloudflared-protocol" class="text-sm font-medium">{{ t('admin.cloudflareTunnel.protocolLabel') }}</Label>
                <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                  {{ t('admin.cloudflareTunnel.protocolDescription') }}
                </p>
              </div>

              <div class="w-full max-w-md space-y-2">
                <Select v-model="protocol">
                  <SelectTrigger id="cloudflared-protocol" class="w-full">
                    <SelectValue :placeholder="t('admin.cloudflareTunnel.protocolPlaceholder')" />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="option in cloudflaredProtocolOptions"
                      :key="option.value"
                      :value="option.value"
                    >
                      {{ option.label }}
                    </SelectItem>
                  </SelectContent>
                </Select>
                <p class="text-xs text-muted-foreground leading-relaxed">
                  {{ cloudflaredProtocolDescription }}
                </p>
              </div>
            </div>
            <div v-if="isReverseProxySubdomainMode" class="p-4 sm:p-6 space-y-4">
              <div class="space-y-1">
                <h3 class="text-sm font-semibold">{{ t('admin.cloudflareTunnel.subdomainChecklistTitle') }}</h3>
                <p class="text-xs leading-relaxed text-muted-foreground">
                  {{ t('admin.cloudflareTunnel.subdomainChecklistDescription') }}
                </p>
              </div>

              <Alert v-if="!hasSubdomainRoot" variant="destructive" class="items-start rounded-xl">
                <TriangleAlert class="h-4 w-4" />
                <AlertTitle>{{ t('admin.cloudflareTunnel.rootMissingTitle') }}</AlertTitle>
                <AlertDescription>
                  {{ t('admin.cloudflareTunnel.rootMissingDescription') }}
                </AlertDescription>
              </Alert>

              <div class="grid gap-3 lg:grid-cols-3">
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">1. Public Hostname</div>
                  <code class="mt-1 block break-all text-sm">{{ publicWildcardHostname }}</code>
                </div>
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">2. Service</div>
                  <code class="mt-1 block break-all text-sm">{{ cloudflaredOriginServiceUrl }}</code>
                </div>
                <div class="rounded-md border bg-muted/20 p-3">
                  <div class="text-xs font-medium text-muted-foreground">3. {{ t('admin.cloudflareTunnel.localAuthHost') }}</div>
                  <code class="mt-1 block break-all text-sm">{{ authServiceHost || t('admin.cloudflareTunnel.notConfigured') }}</code>
                </div>
              </div>

              <div class="space-y-2 text-xs leading-relaxed text-muted-foreground">
                <p>
                  {{ t('admin.cloudflareTunnel.serviceHint') }}
                </p>
              </div>
            </div>
          </div>
        </template>

        <template #actions="{ collapse }">
          <div class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg">
            <Button variant="outline" @click="collapse">{{ t('admin.cloudflareTunnel.collapse') }}</Button>
            <Button :disabled="isSaving" @click="saveConfig" class="min-w-[100px] shadow-sm">{{ t('common.save') }}</Button>
          </div>
        </template>
      </ConfigCollapsibleCard>
    </div>
    <Card>
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>{{ t('admin.cloudflareTunnel.runtimeStatus') }}</CardTitle>
          <Button variant="outline" size="sm" :disabled="isClearingLogs || logs.length === 0" @click="onClearLogsClick">
            <Trash2 class="h-3.5 w-3.5 mr-1" />
            {{ t('admin.cloudflareTunnel.clear') }}
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="text-sm mb-4">
          <span class="mr-4">{{ t('admin.cloudflareTunnel.statusLabel') }}<span :class="running ? 'text-green-600' : 'text-muted-foreground'">{{ running ? t('common.active') :
              t('admin.cloudflareTunnel.notRunning') }}</span></span>
          <span v-if="pid">PID：{{ pid }}</span>
        </div>
        <Alert v-if="cloudflaredLogAnalysis" variant="destructive" class="mb-4 items-start rounded-xl">
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>{{ t('admin.cloudflareTunnel.tlsMismatchTitle') }}</AlertTitle>
          <AlertDescription>
            <div class="grid gap-2">
              <p>{{ cloudflaredLogAnalysisMessage }}</p>
              <ul class="list-disc space-y-1 pl-5">
                <li>{{ t('admin.cloudflareTunnel.tlsMismatchAdviceDisableTls') }}</li>
                <li>{{ t('admin.cloudflareTunnel.tlsMismatchAdviceUseHttp') }}</li>
              </ul>
              <div class="rounded-md border border-current/15 bg-background/60 px-3 py-2 font-mono text-xs break-all">
                {{ cloudflaredLogAnalysis.evidence }}
              </div>
            </div>
          </AlertDescription>
        </Alert>
        <LogViewer :logs="logs" reversed wrap :show-header="false" />
      </CardContent>
    </Card>
    <Dialog v-model:open="showInitDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('admin.cloudflareTunnel.notInitializedTitle') }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">{{ t('admin.cloudflareTunnel.notInitializedDescription') }}</p>
        <DialogFooter>
          <Button @click="gotoResources">{{ t('admin.cloudflareTunnel.goInitialize') }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
