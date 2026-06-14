<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import {
  ConfigAPI,
  FrpcAPI,
  SystemAPI,
  type FrpcInstanceStatus,
  type FrpcInstanceSummary,
  type FrpcInstancesOverview,
} from '../../lib/api'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Info, Pencil, Play, Plus, ScrollText, Square, Trash2 } from 'lucide-vue-next'
import { toast } from '@admin-shared/utils/toast'
import LogViewer from '@admin-shared/components/LogViewer.vue'
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue'
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue'
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../../composables/useTargetPolling'
import { useConfigStore } from '../../store/config'
import DocsLinkButton from '../../components/DocsLinkButton.vue'
import LiveStatusBadge from '../../components/LiveStatusBadge.vue'
import { extractVisualFieldsFromToml } from '../../lib/frpc-config-editor'
import { docsUrls } from '../../lib/docs'
import FrpcInstanceEditor from './frp/FrpcInstanceEditor.vue'

withDefaults(defineProps<{
  showDocsButton?: boolean
}>(), {
  showDocsButton: false,
})

type FrpcEditorExpose = {
  getContent: () => string
  resetFromRaw: (raw: string) => void
}

const router = useRouter()
const configStore = useConfigStore()
const { t } = useI18n()

const overview = ref<FrpcInstancesOverview | null>(null)
const primaryConfig = ref('')
const primaryLogs = ref<string[]>([])
const showInitDialog = ref(false)
const configLoaded = ref(false)
const primaryEditorRef = ref<FrpcEditorExpose | null>(null)
const startingInstanceId = ref<string | null>(null)
const stoppingInstanceId = ref<string | null>(null)
const deletingInstanceId = ref<string | null>(null)

const defaults = computed(() => overview.value?.defaults ?? { local_port: '7999' })
const primaryInstance = computed(() =>
  overview.value?.items.find((item) => item.id === overview.value?.primaryInstanceId) ?? null,
)
const extraInstances = computed(() =>
  overview.value?.items.filter((item) => !item.isPrimary) ?? [],
)
const isInit = computed(() => overview.value?.initialized ?? false)
const running = computed(() => primaryInstance.value?.running ?? false)
const pid = computed(() => primaryInstance.value?.pid ?? null)
const canStart = computed(() => isInit.value && !running.value)
const canStop = computed(() => running.value)
const primarySummary = computed(() =>
  primaryInstance.value?.summary ?? summarizeContent(primaryConfig.value),
)

const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.frpTunnel.saveFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.saveFailed')) })
  },
})
const { isPending: isStarting, run: runStartFrpc } = useAsyncAction()
const { isPending: isStopping, run: runStopFrpc } = useAsyncAction()
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.frpTunnel.clearLogsFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.clearLogsFailed')) })
  },
})
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.frpTunnel.loadStatusFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.loadStatusFailed')) })
  },
})
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.frpTunnel.loadConfigFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.loadConfigFailed')) })
  },
})

const startErrorTrace = ref<{
  pid: number
  markerSeen: boolean
  expireAt: number
} | null>(null)
const START_ERROR_WATCH_MS = 30_000
const CONNECTION_REFUSED_REGEX = /\bconnection refused\b/i

function summarizeContent(raw: string): FrpcInstanceSummary {
  try {
    const fields = extractVisualFieldsFromToml(raw, { localPort: defaults.value.local_port })
    return {
      serverAddr: fields.serverAddr,
      serverPort: fields.serverPort,
      localPort: fields.localPort,
      remotePort: fields.remotePort,
    }
  } catch {
    return {
      serverAddr: '',
      serverPort: '7000',
      localPort: defaults.value.local_port,
      remotePort: '',
    }
  }
}

function formatSummary(summary: FrpcInstanceSummary) {
  const server = summary.serverAddr ? `${summary.serverAddr}:${summary.serverPort || '7000'}` : t('admin.frpTunnel.notConfigured')
  const local = summary.localPort || defaults.value.local_port
  const remote = summary.remotePort || '0'
  return t('admin.frpTunnel.summary', { server, local, remote })
}

function getInstanceDisplayName(instance: FrpcInstanceStatus | null | undefined) {
  if (!instance) return t('admin.frpTunnel.instance')
  const name = instance.name.trim()
  if (name) return name
  if (instance.summary.serverAddr) return `${instance.summary.serverAddr}:${instance.summary.serverPort || '7000'}`
  return instance.isPrimary ? t('admin.frpTunnel.primaryFrp') : t('admin.frpTunnel.instance')
}

function updateOverviewItem(item: FrpcInstanceStatus) {
  if (!overview.value) return
  overview.value = {
    ...overview.value,
    items: overview.value.items.map((current) => (current.id === item.id ? item : current)),
    runningCount: overview.value.items.reduce(
      (count, current) => count + (current.id === item.id ? Number(item.running) : Number(current.running)),
      0,
    ),
  }
}

function gotoInstanceCreate() {
  router.push({ path: '/tunnel/frp/instances/new' })
}

function gotoInstanceDetail(instance: FrpcInstanceStatus, section?: 'config' | 'logs') {
  router.push({
    path: `/tunnel/frp/instances/${encodeURIComponent(instance.id)}`,
    query: section ? { section } : undefined,
  })
}

async function loadStatus() {
  await runLoadStatus(async () => {
    const data = await FrpcAPI.getInstances()
    overview.value = data
    if (!data.initialized) {
      const sys = await SystemAPI.getFrpStatus()
      if (!sys?.data?.downloaded) {
        showInitDialog.value = true
      }
    }
  })
}

async function loadConfig() {
  await runLoadConfig(
    async () => {
      const raw = await FrpcAPI.getConfig()
      primaryConfig.value = raw
      primaryEditorRef.value?.resetFromRaw(raw)
    },
    {
      onFinally: () => {
        configLoaded.value = true
      },
    },
  )
}

async function saveConfig() {
  await runSaveConfig(async () => {
    const content = primaryEditorRef.value?.getContent() ?? primaryConfig.value
    const shouldRestart = running.value
    await FrpcAPI.saveConfig(content)
    primaryConfig.value = content
    if (shouldRestart) {
      await FrpcAPI.stop()
      const res = await FrpcAPI.start()
      startErrorTrace.value = {
        pid: res.pid,
        markerSeen: false,
        expireAt: Date.now() + START_ERROR_WATCH_MS,
      }
      toast.success(t('admin.frpTunnel.restartSuccess'))
    } else {
      toast.success(t('admin.frpTunnel.saveSuccess'))
    }
    await loadStatus()
  })
}

async function startFrpc(options?: { silent?: boolean }) {
  await runStartFrpc(
    () => FrpcAPI.start(),
    {
      onSuccess: async (res) => {
        startErrorTrace.value = {
          pid: res.pid,
          markerSeen: false,
          expireAt: Date.now() + START_ERROR_WATCH_MS,
        }
        await ConfigAPI.updateDefaultTunnel('frp')
        if (configStore.config) {
          configStore.config.default_tunnel = 'frp'
        }
        await loadStatus()
        if (!options?.silent) toast.success(t('admin.frpTunnel.startSuccess'))
      },
      onError: (error) => {
        if (options?.silent) return
        const message = extractErrorMessage(error, t('admin.frpTunnel.startFailed'))
        if (CONNECTION_REFUSED_REGEX.test(message)) {
          toast.error(t('admin.frpTunnel.startFailed'), { description: t('admin.frpTunnel.connectionRefused') })
          return
        }
        toast.error(t('admin.frpTunnel.startFailed'), { description: message })
      },
    },
  )
}

async function stopFrpc(options?: { silent?: boolean }) {
  await runStopFrpc(
    () => FrpcAPI.stop(),
    {
      onSuccess: async () => {
        await loadStatus()
        if (!options?.silent) toast.success(t('admin.frpTunnel.stopSuccess'))
      },
      onError: (error) => {
        if (options?.silent) return
        toast.error(t('admin.frpTunnel.stopFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.stopFailed')) })
      },
    },
  )
}

async function onClearLogsClick() {
  await runClearLogs(
    () => FrpcAPI.clearLogs(),
    {
      onSuccess: () => {
        primaryLogs.value = []
        frpcPolling.resetCursor()
        void frpcPolling.refresh()
        toast.success(t('admin.frpTunnel.logsCleared'))
      },
    },
  )
}

async function startInstance(instance: FrpcInstanceStatus) {
  if (startingInstanceId.value) return
  startingInstanceId.value = instance.id
  try {
    await FrpcAPI.startInstance(instance.id)
    await ConfigAPI.updateDefaultTunnel('frp')
    if (configStore.config) {
      configStore.config.default_tunnel = 'frp'
    }
    toast.success(t('admin.frpTunnel.startSuccess'))
    await loadStatus()
  } catch (error) {
    toast.error(t('admin.frpTunnel.startFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.startFailed')) })
  } finally {
    startingInstanceId.value = null
  }
}

async function stopInstance(instance: FrpcInstanceStatus) {
  if (stoppingInstanceId.value) return
  stoppingInstanceId.value = instance.id
  try {
    await FrpcAPI.stopInstance(instance.id)
    toast.success(t('admin.frpTunnel.stopSuccess'))
    await loadStatus()
  } catch (error) {
    toast.error(t('admin.frpTunnel.stopFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.stopFailed')) })
  } finally {
    stoppingInstanceId.value = null
  }
}

async function deleteInstance(instance: FrpcInstanceStatus) {
  if (deletingInstanceId.value) return
  deletingInstanceId.value = instance.id
  try {
    await FrpcAPI.deleteInstance(instance.id)
    toast.success(t('admin.frpTunnel.instanceDeleted'))
    await loadStatus()
  } catch (error) {
    toast.error(t('admin.frpTunnel.deleteFailed'), { description: extractErrorMessage(error, t('admin.frpTunnel.deleteFailed')) })
  } finally {
    deletingInstanceId.value = null
  }
}

function gotoFrpResources() {
  showInitDialog.value = false
  router.push({ path: '/system', query: { tab: 'frp' } })
}

function handleStartFailureLogs(lines: string[]) {
  const trace = startErrorTrace.value
  if (!trace) return
  if (Date.now() > trace.expireAt) {
    startErrorTrace.value = null
    return
  }

  for (const line of lines) {
    const text = line.trim()
    if (!text) continue
    if (!trace.markerSeen && text.includes(`frpc started pid=${trace.pid}`)) {
      trace.markerSeen = true
      continue
    }
    if (!trace.markerSeen) continue
    if (!CONNECTION_REFUSED_REGEX.test(text)) continue
    toast.error(t('admin.frpTunnel.startFailed'), { description: t('admin.frpTunnel.connectionRefused') })
    startErrorTrace.value = null
    return
  }
}

const frpcPolling = useTargetPolling({
  target: 'frpc',
  intervalMs: 2000,
  onData: (payload) => {
    primaryLogs.value = mergePollingLogWindow(primaryLogs.value, payload.logs, {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    })

    if (payload.status.instances) {
      overview.value = payload.status.instances
    } else {
      updateOverviewItem(payload.status)
    }
    handleStartFailureLogs(payload.logs)
  },
})

onMounted(async () => {
  await loadStatus()
  await loadConfig()
  frpcPolling.start()
})
onUnmounted(() => {
  frpcPolling.stop()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
      <div class="space-y-1">
        <h2 class="text-xl font-semibold">{{ t('admin.frpTunnel.title') }}</h2>
        <p class="text-sm text-muted-foreground">
          {{ t('admin.frpTunnel.runningSummary', { running: overview?.runningCount ?? 0, total: overview?.total ?? 0 }) }}
        </p>
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <DocsLinkButton
          v-if="showDocsButton"
          :href="docsUrls.guides.tunnel"
          size="default"
          class="shrink-0"
        />
        <Button v-if="!running" :disabled="!canStart || isStarting" @click="startFrpc">
          <Play class="mr-1.5 h-4 w-4" />
          {{ t('admin.frpTunnel.start') }}
        </Button>
        <Button v-else variant="destructive" :disabled="!canStop || isStopping" @click="stopFrpc">
          <Square class="mr-1.5 h-4 w-4" />
          {{ t('admin.frpTunnel.stop') }}
        </Button>
      </div>
    </div>

    <ConfigCollapsibleCard
      :title="t('admin.frpTunnel.primaryConfigTitle')"
      :configured="Boolean(primarySummary.serverAddr)"
      :ready="configLoaded"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{ formatSummary(primarySummary) }}
      </template>

      <template #default>
        <FrpcInstanceEditor
          ref="primaryEditorRef"
          v-model="primaryConfig"
          :defaults="defaults"
          id-prefix="frp-primary"
        />
      </template>

      <template #actions="{ collapse }">
        <div class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg">
          <Button variant="outline" @click="collapse">{{ t('admin.frpTunnel.collapse') }}</Button>
          <Button :disabled="isSaving" @click="saveConfig" class="min-w-[100px] shadow-sm">{{ t('common.save') }}</Button>
        </div>
      </template>
    </ConfigCollapsibleCard>

    <Card>
      <CardHeader>
        <div class="flex items-center justify-between gap-3">
          <CardTitle class="text-base">{{ t('admin.frpTunnel.primaryConnectionInfo') }}</CardTitle>
          <Button variant="outline" size="sm" :disabled="isClearingLogs || primaryLogs.length === 0" @click="onClearLogsClick">
            <Trash2 class="h-3.5 w-3.5 mr-1" />
            {{ t('admin.frpTunnel.clear') }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-4">
        <div class="grid gap-3 text-sm sm:grid-cols-3">
          <div>
            <div class="text-xs text-muted-foreground">{{ t('admin.frpTunnel.status') }}</div>
            <div class="mt-1 flex items-center gap-2">
              <LiveStatusBadge :active="running" />
              <span :class="running ? 'text-green-600' : 'text-muted-foreground'">
                {{ running ? t('common.active') : t('admin.frpTunnel.notRunning') }}
              </span>
            </div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">PID</div>
            <div class="mt-1 font-mono">{{ pid ?? '-' }}</div>
          </div>
          <div>
            <div class="text-xs text-muted-foreground">{{ t('admin.frpTunnel.logAttachment') }}</div>
            <div class="mt-1">{{ primaryInstance?.attached ? t('admin.frpTunnel.currentProcess') : t('admin.frpTunnel.historyBuffer') }}</div>
          </div>
        </div>
        <LogViewer :logs="primaryLogs" reversed :show-header="false" />
      </CardContent>
    </Card>

    <Card class="gap-2">
      <CardHeader>
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
          <div class="space-y-1">
            <CardTitle class="text-base">{{ t('admin.frpTunnel.moreFrp') }}</CardTitle>
            <p class="text-sm text-muted-foreground">
              {{ t('admin.frpTunnel.moreFrpDescription') }}
            </p>
          </div>
          <Button size="sm" @click="gotoInstanceCreate">
            <Plus class="mr-1.5 h-4 w-4" />
            {{ t('admin.frpTunnel.addFrp') }}
          </Button>
        </div>
      </CardHeader>
      <CardContent class="space-y-3">
        <div
          v-if="extraInstances.length === 0"
          class="rounded-lg border border-dashed px-4 py-6 text-sm text-muted-foreground"
        >
          {{ t('admin.frpTunnel.emptyExtra') }}
        </div>

        <div v-else class="space-y-3">
          <div
            v-for="instance in extraInstances"
            :key="instance.id"
            class="rounded-lg border bg-card px-4 py-4"
          >
            <div class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between">
              <div class="min-w-0 space-y-2">
                <div class="flex flex-wrap items-center gap-2">
                  <p class="text-sm font-medium">{{ getInstanceDisplayName(instance) }}</p>
                  <span class="inline-flex items-center gap-1.5 text-xs" :class="instance.running ? 'text-green-600' : 'text-muted-foreground'">
                    <LiveStatusBadge :active="instance.running" size="xs" />
                    {{ instance.running ? t('common.active') : t('admin.frpTunnel.notRunning') }}
                  </span>
                </div>
                <p class="text-sm text-muted-foreground break-all">
                  {{ formatSummary(instance.summary) }}
                </p>
                <p v-if="instance.lastMessage" class="text-xs text-muted-foreground">
                  {{ instance.lastMessage }}
                </p>
              </div>

              <div class="grid gap-3 sm:grid-cols-3 lg:min-w-[360px]">
                <div class="rounded-lg px-3 py-2">
                  <p class="text-[10px] uppercase tracking-wider text-muted-foreground">PID</p>
                  <p class="mt-1 font-mono text-sm">{{ instance.pid ?? '-' }}</p>
                </div>
                <div class="rounded-lg px-3 py-2">
                  <p class="text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('admin.frpTunnel.lastStarted') }}</p>
                  <p class="mt-1 text-sm">
                    <HumanFriendlyTime :value="instance.startedAt" />
                  </p>
                </div>
                <div class="rounded-lg px-3 py-2">
                  <p class="text-[10px] uppercase tracking-wider text-muted-foreground">{{ t('admin.frpTunnel.logs') }}</p>
                  <p class="mt-1 text-sm">{{ instance.attached ? t('admin.frpTunnel.liveAttached') : t('admin.frpTunnel.historyBuffer') }}</p>
                </div>
              </div>
            </div>

            <div class="mt-4 flex flex-wrap justify-end gap-2">
              <Button variant="outline" size="sm" @click="gotoInstanceDetail(instance, 'config')">
                <Pencil class="mr-1.5 h-3.5 w-3.5" />
                {{ t('admin.frpTunnel.edit') }}
              </Button>
              <Button
                v-if="!instance.running"
                variant="outline"
                size="sm"
                :disabled="startingInstanceId === instance.id"
                @click="startInstance(instance)"
              >
                <Play class="mr-1.5 h-3.5 w-3.5" />
                {{ startingInstanceId === instance.id ? t('admin.frpTunnel.starting') : t('admin.frpTunnel.start') }}
              </Button>
              <Button
                v-else
                variant="outline"
                size="sm"
                :disabled="stoppingInstanceId === instance.id"
                @click="stopInstance(instance)"
              >
                <Square class="mr-1.5 h-3.5 w-3.5" />
                {{ stoppingInstanceId === instance.id ? t('admin.frpTunnel.stopping') : t('admin.frpTunnel.stop') }}
              </Button>
              <Button variant="outline" size="sm" @click="gotoInstanceDetail(instance, 'logs')">
                <ScrollText class="mr-1.5 h-3.5 w-3.5" />
                {{ t('admin.frpTunnel.logs') }}
              </Button>
              <Button variant="outline" size="sm" @click="gotoInstanceDetail(instance)">
                <Info class="mr-1.5 h-3.5 w-3.5" />
                {{ t('admin.frpTunnel.viewMore') }}
              </Button>
              <ConfirmDangerPopover
                :title="t('admin.frpTunnel.deleteTitle')"
                :description="t('admin.frpTunnel.deleteDescription', { name: getInstanceDisplayName(instance) })"
                :loading="deletingInstanceId === instance.id"
                :disabled="deletingInstanceId === instance.id"
                :on-confirm="() => deleteInstance(instance)"
                content-class="w-72 text-left"
              >
                <template #trigger>
                  <Button
                    variant="outline"
                    size="sm"
                    :disabled="deletingInstanceId === instance.id"
                    class="text-destructive hover:text-destructive"
                  >
                    <Trash2 class="mr-1.5 h-3.5 w-3.5" />
                    {{ deletingInstanceId === instance.id ? t('admin.frpTunnel.deleting') : t('admin.frpTunnel.delete') }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
            </div>
          </div>
        </div>
      </CardContent>
    </Card>

    <Dialog v-model:open="showInitDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('admin.frpTunnel.notInitializedTitle') }}</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">{{ t('admin.frpTunnel.notInitializedDescription') }}</p>
        <DialogFooter>
          <Button @click="gotoFrpResources">{{ t('admin.frpTunnel.goInitialize') }}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
