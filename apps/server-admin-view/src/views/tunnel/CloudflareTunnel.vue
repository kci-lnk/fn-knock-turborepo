<script setup lang="ts">
import { onMounted, onUnmounted, ref, computed, watch } from 'vue'
import { useRouter } from 'vue-router'
import { CloudflaredAPI, SystemAPI, ConfigAPI } from '../../lib/api'
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogFooter } from '@/components/ui/dialog'
import { EyeIcon, EyeOffIcon, Trash2 } from 'lucide-vue-next'
import { toast } from '@admin-shared/utils/toast'
import LogViewer from '@admin-shared/components/LogViewer.vue'
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue'
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction'
import { DEFAULT_LOG_WINDOW_SIZE, mergePollingLogWindow } from '@admin-shared/utils/log-window'
import { useTargetPolling } from '../../composables/useTargetPolling'

const router = useRouter()

const isInit = ref<boolean>(false)
const running = ref<boolean>(false)
const pid = ref<number | null>(null)
const logs = ref<string[]>([])
const showInitDialog = ref(false)
const showToken = ref(true)
const configLoaded = ref(false)

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
