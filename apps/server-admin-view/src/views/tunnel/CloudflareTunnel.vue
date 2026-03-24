<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { CloudflaredAPI, SystemAPI, ConfigAPI } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
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

const token = ref<string>('')
const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  onError: (error) => {
    toast.error('保存失败', { description: extractErrorMessage(error, '保存失败') })
  },
})
const { isPending: isStarting, run: runStartCloudflared } = useAsyncAction()
const { isPending: isStopping, run: runStopCloudflared } = useAsyncAction()
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

watch(token, (newVal) => {
  if (!newVal) return
  // Token normally starts with eyJ and is a base64 encoded JSON string over ~100 characters long
  const rawTokenMatch = newVal.match(/(eyJ[A-Za-z0-9-_]+)/)
  if (rawTokenMatch && rawTokenMatch[1]) {
    const extracted = rawTokenMatch[1]
    if (newVal !== extracted) {
      token.value = extracted
      toast.success('已自动提取 Token')
    }
  }
})

const canStart = computed(() => isInit.value && !running.value && token.value)
const canStop = computed(() => running.value)
const cloudflaredLogAnalysisMessage = computed(() => {
  const analysis = cloudflaredLogAnalysis.value
  if (!analysis) return ''

  const certificateTargets = analysis.certificateHosts.join('、')
  const originTarget = analysis.originHost
    ? `当前 Tunnel 正在回源到 ${analysis.originHost}`
    : '当前 Tunnel 回源时'

  return `${originTarget}，日志里的 “certificate is valid for” 表示证书只适用于 ${certificateTargets}，但 cloudflared 实际校验的主机名是 ${analysis.requestedHost}。这通常说明源站证书配置不正确。`
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
    await CloudflaredAPI.saveConfig(token.value.trim())
    const shouldRestart = running.value
    if (shouldRestart) {
      await stopCloudflared({ silent: true })
      await startCloudflared({ silent: true })
      toast.success('重启成功')
      return
    }
    toast.success('保存成功')
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
        if (!options?.silent) toast.success('启动成功')
      },
      onError: (error) => {
        if (options?.silent) return
        toast.error('启动失败', { description: extractErrorMessage(error, '启动失败') })
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
    () => CloudflaredAPI.clearLogs(),
    {
      onSuccess: () => {
        logs.value = []
        cloudflaredLogAnalysis.value = null
        cloudflaredPolling.resetCursor()
        void cloudflaredPolling.refresh()
        toast.success('日志已清空')
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
  await loadStatus()
  await loadConfig()
  cloudflaredPolling.start()
})
onUnmounted(() => {
  cloudflaredPolling.stop()
})
</script>

<template>
  <div class="space-y-6">
    <div class="flex items-center justify-between">
      <h2 class="text-xl font-semibold">Cloudflared 穿透</h2>
      <div class="flex gap-2">
        <Button v-if="!running" :disabled="!canStart || isStarting" @click="startCloudflared">启动</Button>
        <Button v-else variant="destructive" :disabled="!canStop || isStopping" @click="stopCloudflared">停止</Button>
      </div>
    </div>

    <div class="grid grid-cols-1">
      <ConfigCollapsibleCard
        title="Cloudflared 配置"
        :configured="Boolean(token)"
        :ready="configLoaded"
        expanded-content-class="p-0 sm:p-0"
      >
        <template #summary>
          Token: {{ token ? '********' : '未配置' }}
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
                  Cloudflare Tunnel 的接入密钥。支持粘贴完整命令，系统会自动提取 token。
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
                  Cloudflare Tunnel 的接入密钥。支持粘贴完整命令，系统会自动提取 token。
                </p>
                <div class="text-xs text-muted-foreground mt-2 space-y-1 leading-relaxed">
                  <p>配置来源: 登录 <a href="https://one.dash.cloudflare.com/" target="_blank" class="text-primary hover:underline font-medium">Cloudflare Zero Trust Dashboard</a></p>
                  <p>进入 <strong>Networks → Tunnels</strong>，新建一个 <strong>Cloudflared</strong> 类型的 Tunnel。</p>
                  <p>在安装页面复制命令中的 Token（<code>--token</code> 后面的随机长字符串）并粘贴到此处。</p>
                </div>
              </div>
            </div>
          </div>
        </template>

        <template #actions="{ collapse }">
          <div class="p-4 sm:px-6 sm:py-4 bg-muted/30 border-t flex items-center justify-end gap-3 rounded-b-lg">
            <Button variant="outline" @click="collapse">折叠</Button>
            <Button :disabled="isSaving" @click="saveConfig" class="min-w-[100px] shadow-sm">保存</Button>
          </div>
        </template>
      </ConfigCollapsibleCard>
    </div>
    <Card>
      <CardHeader>
        <div class="flex items-center justify-between">
          <CardTitle>运行状态</CardTitle>
          <Button variant="outline" size="sm" :disabled="isClearingLogs || logs.length === 0" @click="onClearLogsClick">
            <Trash2 class="h-3.5 w-3.5 mr-1" />
            清空
          </Button>
        </div>
      </CardHeader>
      <CardContent>
        <div class="text-sm mb-4">
          <span class="mr-4">状态：<span :class="running ? 'text-green-600' : 'text-muted-foreground'">{{ running ? '运行中' :
              '未运行' }}</span></span>
          <span v-if="pid">PID：{{ pid }}</span>
        </div>
        <Alert v-if="cloudflaredLogAnalysis" variant="destructive" class="mb-4 items-start rounded-xl">
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>检测到源站 TLS 证书域名不匹配</AlertTitle>
          <AlertDescription>
            <div class="grid gap-2">
              <p>{{ cloudflaredLogAnalysisMessage }}</p>
              <ul class="list-disc space-y-1 pl-5">
                <li>建议在 Cloudflare Tunnel 中关闭 TLS 验证，避免 cloudflared 继续校验源站证书。</li>
                <li>也可以选择删除证书，并在 Cloudflare 的 Tunnel 配置里将回源协议改为 HTTP。</li>
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
          <DialogTitle>Cloudflared 未初始化</DialogTitle>
        </DialogHeader>
        <p class="text-sm text-muted-foreground">请先在 系统设置 → 其他资源 中完成安装。</p>
        <DialogFooter>
          <Button @click="gotoResources">前往初始化</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>
