<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  Cable,
  ShieldAlert,
  TerminalSquare,
  TriangleAlert,
} from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, TerminalAPI } from "../../lib/api";
import type { TerminalFeatureConfig, TerminalRuntimeStatus } from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { useConfigStore } from "../../store/config";

const configStore = useConfigStore();
const settings = ref<TerminalFeatureConfig | null>(null);
const runtimeStatus = ref<TerminalRuntimeStatus | null>(null);
const form = reactive<TerminalFeatureConfig>({
  enabled: false,
  default_shell: "",
  default_cwd: "",
  max_sessions: 3,
  idle_timeout_seconds: 24 * 60 * 60,
  resume_backend: "tmux",
  allow_mobile_toolbar: true,
  dangerously_run_as_current_user: false,
});

const { isPending: isLoading, run: runLoad } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取网页终端设置"),
    });
  },
});
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "网页终端设置保存失败"),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);

const isDirty = computed(() => {
  if (!settings.value) return false;
  return JSON.stringify(settings.value) !== JSON.stringify(form);
});

const applyFromSettings = (data: TerminalFeatureConfig) => {
  settings.value = data;
  form.enabled = data.enabled;
  form.default_shell = data.default_shell;
  form.default_cwd = data.default_cwd;
  form.max_sessions = data.max_sessions;
  form.idle_timeout_seconds = data.idle_timeout_seconds;
  form.resume_backend = "tmux";
  form.allow_mobile_toolbar = data.allow_mobile_toolbar;
  form.dangerously_run_as_current_user = data.dangerously_run_as_current_user;
};

const loadData = async () => {
  await runLoad(async () => {
    const [config, status] = await Promise.all([
      ConfigAPI.getTerminalFeature(),
      TerminalAPI.getStatus(),
    ]);
    applyFromSettings(config);
    runtimeStatus.value = status;
  });
};

const saveSettings = async () => {
  await runSave(
    () =>
      ConfigAPI.updateTerminalFeature({
        enabled: form.enabled,
        default_shell: form.default_shell.trim(),
        default_cwd: form.default_cwd.trim(),
        max_sessions: Math.max(1, Math.floor(Number(form.max_sessions) || 1)),
        idle_timeout_seconds: Math.max(
          60,
          Math.floor(Number(form.idle_timeout_seconds) || 60),
        ),
        resume_backend: "tmux",
        allow_mobile_toolbar: form.allow_mobile_toolbar,
        dangerously_run_as_current_user: form.dangerously_run_as_current_user,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await Promise.all([configStore.loadConfig(), loadData()]);
        toast.success("网页终端设置已更新");
      },
    },
  );
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

onMounted(loadData);
</script>

<template>
  <Card>
    <CardHeader>
      <div class="flex items-start justify-between gap-3">
        <div class="space-y-1.5">
          <CardTitle class="text-md">网页终端</CardTitle>
          <CardDescription>
            基于 <code>tmux</code> 持久化终端会话，默认使用 HTTP
            长轮询同步屏幕状态。
          </CardDescription>
        </div>
        <Badge variant="outline" class="gap-1.5">
          <TerminalSquare class="h-3.5 w-3.5" />
          <span>tmux</span>
        </Badge>
      </div>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-40" />
        <Skeleton class="h-20 w-full" />
      </div>
    </CardContent>

    <CardContent v-else class="border-t p-0">
      <div class="space-y-6 p-6">
        <Alert
          v-if="runtimeStatus?.blockedReason"
          variant="destructive"
          class="border-destructive/40"
        >
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>当前不可创建终端</AlertTitle>
          <AlertDescription>{{ runtimeStatus.blockedReason }}</AlertDescription>
        </Alert>

        <Alert
          v-else-if="runtimeStatus?.runningAsRoot"
          class="border-amber-500/30 bg-amber-50 text-amber-950"
        >
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>当前进程以 root 运行</AlertTitle>
          <AlertDescription>
            若启用“以当前进程用户运行”，网页终端将拥有系统级权限，请谨慎开放。
          </AlertDescription>
        </Alert>

        <div class="grid gap-3 md:grid-cols-3">
          <div class="rounded-lg border bg-muted/20 p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium">
              <Cable class="h-4 w-4" />
              <span>运行状态</span>
            </div>
            <div class="space-y-2 text-sm text-muted-foreground">
              <p>
                <span class="text-foreground">tmux：</span>
                {{ runtimeStatus?.tmuxAvailable ? "已检测到" : "未检测到" }}
              </p>
              <p>
                <span class="text-foreground">HTTP 长轮询：</span>
                {{ runtimeStatus?.httpPollingAvailable ? "支持" : "不可用" }}
              </p>
            </div>
          </div>

          <div class="rounded-lg border bg-muted/20 p-4">
            <div class="mb-2 text-sm font-medium">默认恢复后端</div>
            <p class="text-sm text-muted-foreground">
              当前固定使用 <code>tmux</code> 作为真实会话承载层，
              以保证刷新页面后还能恢复屏幕状态。
            </p>
          </div>

          <div class="rounded-lg border bg-muted/20 p-4">
            <div class="mb-2 text-sm font-medium">连接策略</div>
            <p class="text-sm text-muted-foreground">
              当前实现固定走 HTTP 长轮询，所有接入场景保持同一条链路。
            </p>
          </div>
        </div>

        <div
          class="flex items-center justify-between rounded-lg border bg-muted/10 p-4"
        >
          <div class="space-y-1 pr-6">
            <Label
              class="cursor-pointer text-base font-medium"
              @click="form.enabled = !form.enabled"
            >
              启用网页终端
            </Label>
            <p class="text-sm text-muted-foreground">
              开启后会在侧边导航显示“网页终端”，并允许创建可恢复会话。
            </p>
          </div>
          <Switch v-model="form.enabled" :disabled="isSaving" />
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="space-y-2">
            <Label for="terminal-shell">默认 Shell</Label>
            <Input
              id="terminal-shell"
              v-model="form.default_shell"
              placeholder="/bin/zsh"
              :disabled="isSaving"
            />
            <p class="text-xs text-muted-foreground">
              为空时会自动退回到当前环境默认 Shell。
            </p>
          </div>
          <div class="space-y-2">
            <Label for="terminal-cwd">默认工作目录</Label>
            <Input
              id="terminal-cwd"
              v-model="form.default_cwd"
              placeholder="/var/lib/fn-knock"
              :disabled="isSaving"
            />
          </div>
          <div class="space-y-2">
            <Label for="terminal-max-sessions">最大会话数</Label>
            <Input
              id="terminal-max-sessions"
              v-model.number="form.max_sessions"
              type="number"
              min="1"
              max="12"
              :disabled="isSaving"
            />
          </div>
          <div class="space-y-2">
            <Label for="terminal-idle-timeout">空闲清理时间（秒）</Label>
            <Input
              id="terminal-idle-timeout"
              v-model.number="form.idle_timeout_seconds"
              type="number"
              min="60"
              step="60"
              :disabled="isSaving"
            />
          </div>
        </div>

        <div class="grid gap-4 md:grid-cols-2">
          <div class="flex items-center justify-between rounded-lg border p-4">
            <div class="space-y-1 pr-4">
              <Label class="text-base">移动端快捷键栏</Label>
              <p class="text-sm text-muted-foreground">
                在手机上显示 Ctrl、Esc、Tab 和方向键快捷按钮。
              </p>
            </div>
            <Switch v-model="form.allow_mobile_toolbar" :disabled="isSaving" />
          </div>

          <div class="flex items-center justify-between rounded-lg border p-4">
            <div class="space-y-1 pr-4">
              <Label class="text-base">以当前进程用户运行</Label>
              <p class="text-sm text-muted-foreground">
                高危选项。若后台是 root，此终端也会继承 root 权限。
              </p>
            </div>
            <Switch
              v-model="form.dangerously_run_as_current_user"
              :disabled="isSaving"
            />
          </div>
        </div>

        <div class="flex items-center justify-end gap-3">
          <Button
            variant="outline"
            :disabled="!isDirty || isSaving"
            @click="resetForm"
          >
            重置
          </Button>
          <Button :disabled="!isDirty || isSaving" @click="saveSettings">
            保存设置
          </Button>
        </div>
      </div>
    </CardContent>
  </Card>
</template>
