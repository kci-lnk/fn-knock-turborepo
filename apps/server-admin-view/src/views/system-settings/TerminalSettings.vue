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
import RefreshButton from "@/components/RefreshButton.vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import {
  ShieldAlert,
  TerminalSquare,
  TriangleAlert,
} from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI, TerminalAPI } from "../../lib/api";
import type {
  TerminalFeatureConfig,
  TerminalRuntimeStatus,
  TerminalTmuxInstallState,
} from "../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePollingResourceStatus } from "@admin-shared/composables/usePollingResourceStatus";
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
  dangerously_run_as_current_user: true,
});

const createEmptyTmuxInstallState = (): TerminalTmuxInstallState => ({
  status: "uninstalled",
  progress: 0,
  message: "未检测到 tmux，请先安装 Debian tmux 环境",
  executablePath: "",
  detectionSource: null,
  version: "",
});

const { isPending: isLoadingConfig, run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    toast.error("加载失败", {
      description: extractErrorMessage(error, "无法获取 Web 终端设置"),
    });
  },
});
const { isPending: isFetchingStatus, run: runFetchStatus } = useAsyncAction();
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error("保存失败", {
      description: extractErrorMessage(error, "Web 终端设置保存失败"),
    });
  },
});
const { isPending: isStartingInstall, run: runStartInstall } = useAsyncAction({
  onError: async (error) => {
    toast.error("安装失败", {
      description: extractErrorMessage(error, "无法启动 tmux 安装"),
    });
    await refreshStatus();
  },
});

const tmuxInstallState = computed(
  () => runtimeStatus.value?.tmuxInstallState ?? createEmptyTmuxInstallState(),
);
const isTmuxInstalled = computed(
  () => tmuxInstallState.value.status === "installed",
);
const isTmuxInstalling = computed(
  () => tmuxInstallState.value.status === "installing",
);
const tmuxProgress = computed(() => {
  const progress = Number(tmuxInstallState.value.progress ?? 0);
  if (!Number.isFinite(progress)) return 0;
  return Math.max(0, Math.min(100, progress));
});
const tmuxStatusLabel = computed(() => {
  const status = tmuxInstallState.value.status;
  if (status === "installed") return "已安装";
  if (status === "installing") return "安装中";
  if (status === "error") return "错误";
  return "未安装";
});
const tmuxStatusVariant = computed(() => {
  const status = tmuxInstallState.value.status;
  if (status === "installed") return "default";
  if (status === "installing") return "secondary";
  if (status === "error") return "destructive";
  return "outline";
});
const showBlockedAlert = computed(() => {
  const reason = runtimeStatus.value?.blockedReason?.trim() || "";
  return Boolean(reason) && reason !== "网页终端功能尚未启用";
});
const isDirty = computed(() => {
  if (!settings.value) return false;
  return JSON.stringify(settings.value) !== JSON.stringify(form);
});

const { isInitializing: isInitializingStatus, refresh: refreshStatus } =
  usePollingResourceStatus<TerminalRuntimeStatus | null>({
    fetcher: async () => {
      const data = await runFetchStatus(() => TerminalAPI.getStatus());
      return data ?? runtimeStatus.value;
    },
    onData: (data) => {
      runtimeStatus.value = data;
    },
    isDownloading: (data) => data?.tmuxInstallState?.status === "installing",
    onError: (error) => {
      if (!runtimeStatus.value) {
        toast.error("加载失败", {
          description: extractErrorMessage(error, "无法获取 tmux 环境状态"),
        });
      }
    },
  });

const isInitialLoading = computed(
  () => isLoadingConfig.value || isInitializingStatus.value,
);
const showLoadingSkeleton = useDelayedLoading(isInitialLoading);

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

const loadSettings = async () => {
  await runLoadConfig(async () => {
    const config = await ConfigAPI.getTerminalFeature();
    applyFromSettings(config);
  });
};


const refreshAll = async () => {
  await Promise.all([loadSettings(), refreshStatus()]);
};

const saveSettings = async () => {
  await runSave(
    () =>
      ConfigAPI.updateTerminalFeature({
        enabled: form.enabled,
        max_sessions: Math.max(1, Math.floor(Number(form.max_sessions) || 1)),
        idle_timeout_seconds: Math.max(
          60,
          Math.floor(Number(form.idle_timeout_seconds) || 60),
        ),
        allow_mobile_toolbar: form.allow_mobile_toolbar,
        dangerously_run_as_current_user: form.dangerously_run_as_current_user,
      }),
    {
      onSuccess: async (data) => {
        applyFromSettings(data);
        await Promise.all([
          configStore.loadConfig(),
          loadSettings(),
          refreshStatus(),
        ]);
        toast.success("Web 终端设置已更新");
      },
    },
  );
};

const resetForm = () => {
  if (settings.value) {
    applyFromSettings(settings.value);
  }
};

const startTmuxInstall = async () => {
  if (isTmuxInstalling.value) return;
  await runStartInstall(() => TerminalAPI.installTmux(), {
    onSuccess: async (state) => {
      toast.success(
        state.status === "installed" ? "tmux 已就绪" : "已开始安装 tmux",
      );
      await refreshStatus();
    },
  });
};

onMounted(loadSettings);
</script>

<template>
  <Card class="w-full">
    <CardHeader>
      <div class="flex items-start justify-between gap-4">
        <div class="grid gap-1">
          <CardTitle class="flex items-center gap-2 text-md">
            <span>Web终端</span>
            <Badge :variant="tmuxStatusVariant">{{ tmuxStatusLabel }}</Badge>
          </CardTitle>
          <CardDescription>
            使用 <code>tmux</code> 承载可恢复的 Web
            终端会话。
          </CardDescription>
        </div>
        <RefreshButton
          :loading="isLoadingConfig || isFetchingStatus"
          :disabled="isSaving || isStartingInstall || isFetchingStatus"
          @click="refreshAll"
        />
      </div>
    </CardHeader>

    <CardContent
      v-if="isInitialLoading && showLoadingSkeleton"
      class="border-t p-0"
    >
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-40" />
        <Skeleton class="h-24 w-full" />
        <Skeleton class="h-32 w-full" />
      </div>
    </CardContent>

    <CardContent v-else class="border-t p-0">
      <div class="space-y-6 p-6">
        <Alert
          v-if="showBlockedAlert"
          variant="destructive"
          class="border-destructive/40"
        >
          <TriangleAlert class="h-4 w-4" />
          <AlertTitle>当前不可创建终端</AlertTitle>
          <AlertDescription>{{
            runtimeStatus?.blockedReason
          }}</AlertDescription>
        </Alert>

        <Alert
          v-else-if="
            runtimeStatus?.runningAsRoot && form.dangerously_run_as_current_user
          "
          class="border-amber-500/30 bg-amber-50 text-amber-950"
        >
          <ShieldAlert class="h-4 w-4" />
          <AlertTitle>当前进程以 root 运行</AlertTitle>
          <AlertDescription>
            当前默认配置会让 Web 终端继承当前进程用户权限。由于后台是 root，
            启用后终端也会拥有 root 权限，请确认这是你期望的行为。
          </AlertDescription>
        </Alert>

        <div class="grid gap-4 md:grid-cols-1">
          <div class="rounded-lg border bg-muted/20 p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium">
              <TerminalSquare class="h-4 w-4" />
              <span>Tmux 环境</span>
            </div>
            <div class="space-y-2 text-sm text-muted-foreground">
              <p>
                <span class="text-foreground">状态：</span>{{ tmuxStatusLabel }}
              </p>
              <p>
                <span class="text-foreground">版本：</span>
                {{
                  runtimeStatus?.tmuxVersion || tmuxInstallState.version || "-"
                }}
              </p>
            </div>
          </div>

        </div>

        <div v-if="!isTmuxInstalled" class="rounded-lg border bg-muted/10 p-4">
          <div class="flex items-start justify-between gap-4">
            <div class="space-y-1">
              <div class="text-sm font-medium">安装 tmux</div>
              <p class="text-sm text-muted-foreground">
                检测到当前 Debian 环境未就绪时，可直接执行
                <code>apt-get update</code> 与
                <code>apt-get install -y tmux</code> 完成安装。
              </p>
            </div>
            <Badge :variant="tmuxStatusVariant">{{ tmuxStatusLabel }}</Badge>
          </div>

          <div class="mt-4 space-y-3">
            <Progress :model-value="tmuxProgress" />
            <p
              :class="[
                'text-sm',
                tmuxInstallState.status === 'error'
                  ? 'text-destructive'
                  : 'text-muted-foreground',
              ]"
            >
              {{ tmuxInstallState.message }}
            </p>
            <Button
              class="w-full md:w-auto"
              :disabled="isStartingInstall || isTmuxInstalling"
              @click="startTmuxInstall"
            >
              <span
                v-if="isStartingInstall || isTmuxInstalling"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              {{
                tmuxInstallState.status === "error"
                  ? "重新安装 tmux"
                  : "安装 tmux"
              }}
            </Button>
          </div>
        </div>

        <template v-else>
          <div
            class="flex items-center justify-between rounded-lg border bg-muted/10 p-4"
          >
            <div class="space-y-1 pr-6">
              <Label
                class="cursor-pointer text-base font-medium"
                @click="form.enabled = !form.enabled"
              >
                启用 Web终端
              </Label>
              <p class="text-sm text-muted-foreground">
                开启后会在侧边导航显示“Web终端”，并允许创建可恢复会话。
              </p>
            </div>
            <Switch v-model="form.enabled" :disabled="isSaving" />
          </div>

          <div class="grid gap-4 md:grid-cols-2">
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
        </template>
      </div>
    </CardContent>
  </Card>
</template>
