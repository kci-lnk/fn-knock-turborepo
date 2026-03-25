<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed } from 'vue'
import { useRouter } from 'vue-router'
import { FrpcAPI, SystemAPI, ConfigAPI } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { Trash2 } from 'lucide-vue-next'
import { toast } from '@admin-shared/utils/toast'
import LogViewer from '@admin-shared/components/LogViewer.vue'
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../../composables/useTargetPolling'
import { useConfigStore } from '../../store/config'
import TomlCodeEditor from '../../components/TomlCodeEditor.vue'
import DocsLinkButton from '../../components/DocsLinkButton.vue'
import {
  extractVisualFieldsFromToml,
  mergeVisualFieldsIntoToml,
  type FrpcVisualFields,
} from '../../lib/frpc-config-editor'
import { docsUrls } from '../../lib/docs'

withDefaults(defineProps<{
  showDocsButton?: boolean
}>(), {
  showDocsButton: false,
})

const router = useRouter()
const configStore = useConfigStore()

const isInit = ref<boolean>(false)
const running = ref<boolean>(false)
const pid = ref<number | null>(null)
const frpcContent = ref<string>('')
const customToml = ref<string>('')
const editorMode = ref<'visual' | 'custom'>('visual')
const visualSyncError = ref<string | null>(null)
const logs = ref<string[]>([])
const showInitDialog = ref(false)
const defaults = ref<{ local_port: string }>({ local_port: '7999' })
const configLoaded = ref(false)

type TcpItem = {
  name: string
  type: string
  status: string
  err: string
  local_addr: string
  plugin: string
  remote_addr: string
}
const tcpItems = ref<TcpItem[]>([])

const serverAddr = ref<string>('')
const serverPort = ref<string>('7000')
const serverToken = ref<string>('')
const webUser = ref<string>('admin')
const webPassword = ref<string>('')
const localPort = ref<string>('7999')
const remotePort = ref<string>('7999')
const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '保存失败') })
  },
})
const { isPending: isStarting, run: runStartFrpc } = useAsyncAction()
const { isPending: isStopping, run: runStopFrpc } = useAsyncAction()
const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
  onError: (error) => {
    toast.error('清空日志失败', { description: extractErrorMessage(error, '清空日志失败') })
  },
})
const { run: runLoadStatus } = useAsyncAction({
  onError: (error) => {
    toast.error('加载状态失败', { description: extractErrorMessage(error, '加载状态失败') })
  },
})
const { run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    toast.error('加载配置失败', { description: extractErrorMessage(error, '加载配置失败') })
  },
})
const startErrorTrace = ref<{
  pid: number
  markerSeen: boolean
  expireAt: number
} | null>(null)
const START_ERROR_WATCH_MS = 30_000
const CONNECTION_REFUSED_REGEX = /\bconnection refused\b/i

const canStart = computed(() => isInit.value && !running.value)
const canStop = computed(() => running.value)
const isCustomMode = computed(() => editorMode.value === 'custom')
const currentModeLabel = computed(() => isCustomMode.value ? '源码模式' : '表单模式')
const currentModeDescription = computed(() =>
  isCustomMode.value
    ? '直接编辑 frpc.toml，保存前会执行 frpc verify。再次点击“自定义”可尝试回到可视化表单。'
    : '可视化模式只覆盖当前已支持字段，其他 TOML 字段会继续保留，不会被清空。',
)

function getVisualDefaults() {
  return {
    localPort: defaults.value.local_port,
  }
}

function getVisualFields(): FrpcVisualFields {
  return {
    serverAddr: serverAddr.value,
    serverPort: serverPort.value,
    serverToken: serverToken.value,
    webUser: webUser.value,
    webPassword: webPassword.value,
    localPort: localPort.value,
    remotePort: remotePort.value,
  }
}

function applyVisualFields(fields: FrpcVisualFields) {
  serverAddr.value = fields.serverAddr
  serverPort.value = fields.serverPort
  serverToken.value = fields.serverToken
  webUser.value = fields.webUser
  webPassword.value = fields.webPassword
  localPort.value = fields.localPort
  remotePort.value = fields.remotePort
}

function syncVisualFieldsFromRaw(raw: string) {
  applyVisualFields(extractVisualFieldsFromToml(raw, getVisualDefaults()))
  visualSyncError.value = null
}

function buildVisualConfig(baseRaw = customToml.value || frpcContent.value): string {
  return mergeVisualFieldsIntoToml(baseRaw, getVisualFields(), getVisualDefaults())
}

function enterCustomMode() {
  try {
    customToml.value = buildVisualConfig(customToml.value || frpcContent.value)
    editorMode.value = 'custom'
    visualSyncError.value = null
  } catch (error) {
    toast.error('无法进入自定义模式', {
      description: extractErrorMessage(error, '当前配置无法转换为可编辑的 TOML'),
    })
  }
}

function exitCustomMode() {
  try {
    syncVisualFieldsFromRaw(customToml.value)
    editorMode.value = 'visual'
  } catch (error) {
    const message = extractErrorMessage(error, '当前 TOML 语法有误')
    visualSyncError.value = message
    toast.error('无法返回可视化模式', {
      description: `${message}。请先修复自定义内容后再切换。`,
    })
  }
}

function toggleCustomMode() {
  if (isCustomMode.value) {
    exitCustomMode()
    return
  }
  enterCustomMode()
}

async function loadStatus() {
  await runLoadStatus(async () => {
    const st = await FrpcAPI.getStatus()
    isInit.value = st.initialized
    running.value = st.running
    pid.value = st.pid
    defaults.value = st.defaults
    if (!isInit.value) {
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
      frpcContent.value = raw
      customToml.value = raw
      try {
        syncVisualFieldsFromRaw(raw)
        editorMode.value = 'visual'
      } catch (error) {
        editorMode.value = 'custom'
        visualSyncError.value = extractErrorMessage(error, '当前 frpc.toml 无法映射到可视化表单')
        toast.info('已切换到自定义模式', {
          description: `${visualSyncError.value}。你可以先在编辑器里修复或继续手动维护配置。`,
        })
      }
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
    const content = isCustomMode.value ? customToml.value : buildVisualConfig()
    await FrpcAPI.saveConfig(content)
    frpcContent.value = content
    customToml.value = content
    try {
      syncVisualFieldsFromRaw(content)
    } catch (error) {
      visualSyncError.value = extractErrorMessage(error, '配置已保存，但暂时无法映射到可视化表单')
      editorMode.value = 'custom'
    }
    const shouldRestart = running.value
    if (shouldRestart) {
      await stopFrpc({ silent: true })
      await startFrpc({ silent: true })
      toast.success('重启成功')
      return
    }
    toast.success('保存成功')
  })
}

async function startFrpc(options?: { silent?: boolean }) {
  await runStartFrpc(
    () => FrpcAPI.start(),
    {
      onSuccess: async (res) => {
        pid.value = res.pid
        running.value = true
        startErrorTrace.value = {
          pid: res.pid,
          markerSeen: false,
          expireAt: Date.now() + START_ERROR_WATCH_MS,
        }
        await ConfigAPI.updateDefaultTunnel('frp')
        if (configStore.config) {
          configStore.config.default_tunnel = 'frp'
        }
        if (!options?.silent) toast.success('启动成功')
      },
      onError: (error) => {
        if (options?.silent) return
        const message = extractErrorMessage(error, '启动失败')
        if (CONNECTION_REFUSED_REGEX.test(message)) {
          toast.error('启动失败', { description: '无法连接到 FRP 服务端（connection refused），请检查服务端地址、端口和服务状态。' })
          return
        }
        toast.error('启动失败', { description: message })
      },
    },
  )
}

async function stopFrpc(options?: { silent?: boolean }) {
  await runStopFrpc(
    () => FrpcAPI.stop(),
    {
      onSuccess: () => {
        running.value = false
        pid.value = null
        if (!options?.silent) toast.success('停止成功')
      },
      onError: (error) => {
        if (options?.silent) return
        toast.error('停止失败', { description: extractErrorMessage(error, '停止失败') })
      },
    },
  )
}

async function onClearLogsClick() {
  await runClearLogs(
    () => FrpcAPI.clearLogs(),
    {
      onSuccess: () => {
        logs.value = []
        frpcPolling.resetCursor()
        void frpcPolling.refresh()
        toast.success('日志已清空')
      },
    },
  )
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
    toast.error('启动失败', { description: '无法连接到 FRP 服务端（connection refused），请检查服务端地址、端口和服务状态。' })
    startErrorTrace.value = null
    return
  }
}

const frpcPolling = useTargetPolling({
  target: 'frpc',
  intervalMs: 2000,
  onData: (payload) => {
    logs.value = mergePollingLogWindow(logs.value, payload.logs, {
      reset: payload.reset,
      max: DEFAULT_LOG_WINDOW_SIZE,
    })

    running.value = payload.status.running
    pid.value = payload.status.pid
    tcpItems.value = payload.status.tcp || []
    handleStartFailureLogs(payload.logs)
    if (startErrorTrace.value?.markerSeen && payload.status.running && (payload.status.tcp?.length || 0) > 0) {
      startErrorTrace.value = null
    }
  },
  onError: () => {
    tcpItems.value = []
    running.value = false
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
    <div class="flex items-center justify-between">
      <h2 class="text-xl font-semibold">FRP穿透</h2>
      <div class="flex items-center gap-3">
        <DocsLinkButton
          v-if="showDocsButton"
          :href="docsUrls.guides.tunnel"
          size="default"
          class="shrink-0"
        />
        <Button v-if="!running" :disabled="!canStart || isStarting" @click="startFrpc">启动</Button>
        <Button v-else variant="destructive" :disabled="!canStop || isStopping" @click="stopFrpc">停止</Button>
      </div>
    </div>

    <div class="grid grid-cols-1 gap-6">
      <ConfigCollapsibleCard
        title="FRP 配置"
        :configured="Boolean(serverAddr)"
        :ready="configLoaded"
        summary-class="text-xs text-muted-foreground"
        expanded-content-class="p-0 sm:p-0"
      >
        <template #summary>
          {{ serverAddr || '未配置' }}:{{ serverPort || '7000' }}
          · 本地 {{ localPort || defaults.local_port }} → 远端 {{ remotePort || '0' }}
        </template>

        <template #default>
          <div>
            <div class="border-b bg-linear-to-r from-muted/40 via-muted/15 to-transparent px-4 py-4 sm:px-6 sm:py-5">
              <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                <div class="space-y-1">
                  <div class="text-sm font-medium tracking-tight">配置编辑方式</div>
                  <p class="max-w-2xl text-xs leading-relaxed text-muted-foreground">
                    {{ currentModeDescription }}
                  </p>
                </div>
                <div
                  class="inline-flex w-fit items-center rounded-full border px-2.5 py-1 text-[11px] font-medium"
                  :class="isCustomMode ? 'border-primary/20 bg-primary/5 text-primary' : 'border-border bg-background/80 text-muted-foreground'"
                >
                  {{ currentModeLabel }}
                </div>
              </div>
            </div>

            <div v-if="isCustomMode" class="space-y-4 p-4 sm:p-6">
              <div
                v-if="visualSyncError"
                class="rounded-xl border border-destructive/20 bg-destructive/5 px-4 py-3 text-sm leading-relaxed text-destructive"
              >
                当前内容还不能切回可视化模式：{{ visualSyncError }}
              </div>
              <div class="rounded-xl border border-dashed border-border/80 bg-muted/20 px-4 py-3 text-xs leading-relaxed text-muted-foreground">
                这里会直接编辑 <code>frpc.toml</code> 原文。保存时会先执行 <code>frpc verify</code>，可视化模式只管理已支持字段。
              </div>
              <TomlCodeEditor v-model="customToml" />
            </div>

            <div v-else class="divide-y divide-border">
              <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
                <div class="space-y-1 mt-1.5">
                  <Label for="frp-server-addr" class="text-sm font-medium flex items-center gap-1">
                    FRP 服务器地址
                    <span class="text-destructive">*</span>
                  </Label>
                  <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                    FRP 服务端域名或 IP。
                  </p>
                </div>
                <div class="w-full max-w-md space-y-2">
                  <Input id="frp-server-addr" v-model.trim="serverAddr" placeholder="example.com" autocomplete="off"
                    autocapitalize="off" autocorrect="off" :spellcheck="false" data-form-type="other"
                    data-1p-ignore="true" data-lpignore="true" data-bwignore="true" />
                  <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                    FRP 服务端域名或 IP。
                  </p>
                </div>
              </div>

              <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
                <div class="space-y-1 mt-1.5">
                  <Label for="frp-server-port" class="text-sm font-medium flex items-center gap-1">
                    FRP 服务器端口
                    <span class="text-destructive">*</span>
                  </Label>
                </div>
                <div class="w-full max-w-md">
                  <Input id="frp-server-port" v-model="serverPort" type="number" autocomplete="off" autocapitalize="off"
                    autocorrect="off" :spellcheck="false" data-form-type="other" data-1p-ignore="true"
                    data-lpignore="true" data-bwignore="true" />
                </div>
              </div>

              <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
                <div class="space-y-1 mt-1.5">
                  <Label for="frp-server-token" class="text-sm font-medium">Token</Label>
                  <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                    可选，需与服务端配置一致。
                  </p>
                </div>
                <div class="w-full max-w-md space-y-2">
                  <Input id="frp-server-token" v-model.trim="serverToken" placeholder="可选" autocomplete="off"
                    autocapitalize="off" autocorrect="off" :spellcheck="false" data-form-type="other"
                    data-1p-ignore="true" data-lpignore="true" data-bwignore="true" />
                  <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                    可选，需与服务端配置一致。
                  </p>
                </div>
              </div>

              <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
                <div class="space-y-1 mt-1.5">
                  <Label for="frp-local-port" class="text-sm font-medium flex items-center gap-1">
                    本地端口
                    <span class="text-destructive">*</span>
                  </Label>
                  <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                    本机服务监听端口，默认 {{ defaults.local_port }}。
                  </p>
                </div>
                <div class="w-full max-w-md space-y-2">
                  <Input id="frp-local-port" v-model="localPort" type="number" :placeholder="defaults.local_port"
                    autocomplete="off" autocapitalize="off" autocorrect="off" :spellcheck="false"
                    data-form-type="other" data-1p-ignore="true" data-lpignore="true" data-bwignore="true" />
                  <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                    默认 {{ defaults.local_port }}。
                  </p>
                </div>
              </div>

              <div class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10">
                <div class="space-y-1 mt-1.5">
                  <Label for="frp-remote-port" class="text-sm font-medium flex items-center gap-1">
                    出网端口
                    <span class="text-destructive">*</span>
                  </Label>
                  <p class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4">
                    需要映射到外网访问的目标端口。
                  </p>
                </div>
                <div class="w-full max-w-md space-y-2">
                  <Input id="frp-remote-port" v-model="remotePort" type="number" autocomplete="off" autocapitalize="off"
                    autocorrect="off" :spellcheck="false" data-form-type="other" data-1p-ignore="true"
                    data-lpignore="true" data-bwignore="true" />
                  <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
                    需要映射到外网访问的目标端口。
                  </p>
                </div>
              </div>
            </div>
          </div>
        </template>

        <template #actions="{ collapse }">
          <div class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg">
            <Button variant="outline" @click="collapse">折叠</Button>
            <Button
              variant="outline"
              :disabled="isSaving"
              :class="isCustomMode ? 'border-primary bg-primary/5 text-primary hover:bg-primary/10' : ''"
              @click="toggleCustomMode"
            >
              自定义
            </Button>
            <Button :disabled="isSaving" @click="saveConfig" class="min-w-[100px] shadow-sm">保存</Button>
          </div>
        </template>
      </ConfigCollapsibleCard>
    </div>
    <Card>
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>连接信息</CardTitle>
          <Button variant="outline" size="sm" :disabled="isClearingLogs || logs.length === 0" @click="onClearLogsClick">
            <Trash2 class="h-3.5 w-3.5 mr-1" />
            清空
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="text-sm mb-2">
          <span class="mr-4">状态：<span :class="running ? 'text-green-600' : 'text-muted-foreground'">{{ running ? '运行中' :
              '未运行' }}</span></span>
          <span>PID：{{ pid ?? '-' }}</span>
        </div>
        <div v-if="tcpItems.length" class="mb-3 rounded-md border divide-y">
          <div v-for="(item, idx) in tcpItems" :key="idx" class="p-3 grid grid-cols-1 md:grid-cols-3 gap-2 text-sm">
            <div>
              <div class="font-medium">{{ item.name }} <span class="text-muted-foreground">({{ item.type }})</span>
              </div>
              <div class="text-xs" :class="item.status === 'running' ? 'text-green-600' : 'text-destructive'">{{
                item.status }}</div>
            </div>
            <div>
              <div class="text-muted-foreground text-xs">本地地址</div>
              <div class="font-mono">{{ item.local_addr }}</div>
            </div>
            <div>
              <div class="text-muted-foreground text-xs">远端地址</div>
              <div class="font-mono">{{ item.remote_addr }}</div>
            </div>
          </div>
        </div>
        <LogViewer :logs="logs" reversed :show-header="false" />
      </CardContent>
    </Card>
    <Dialog v-model:open="showInitDialog">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>FRP 未初始化</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">请先在 系统设置 → FRP资源 中完成初始化。</p>
        <DialogFooter>
          <Button @click="gotoFrpResources">前往初始化</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
