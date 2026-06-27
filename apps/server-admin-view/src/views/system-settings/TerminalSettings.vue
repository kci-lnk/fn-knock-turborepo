<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Progress } from "@/components/ui/progress";
import { Skeleton } from "@/components/ui/skeleton";
import { Switch } from "@/components/ui/switch";
import { TerminalSquare } from "lucide-vue-next";
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
const { t } = useI18n();
const settings = ref<TerminalFeatureConfig | null>(null);
const runtimeStatus = ref<TerminalRuntimeStatus | null>(null);
const form = reactive<TerminalFeatureConfig>({
  enabled: false,
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
  message: t("admin.terminalSettings.emptyTmuxMessage"),
  executablePath: "",
  detectionSource: null,
  version: "",
});

const { isPending: isLoadingConfig, run: runLoadConfig } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.terminalSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.terminalSettings.loadConfigDescription"),
      ),
    });
  },
});
const { isPending: isFetchingStatus, run: runFetchStatus } = useAsyncAction();
const { isPending: isSaving, run: runSave } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.terminalSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.terminalSettings.saveDescription"),
      ),
    });
  },
});
const { isPending: isStartingInstall, run: runStartInstall } = useAsyncAction({
  onError: async (error) => {
    toast.error(t("admin.terminalSettings.installFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.terminalSettings.installDescription"),
      ),
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
  if (status === "installed") return t("admin.terminalSettings.statusInstalled");
  if (status === "installing") return t("admin.terminalSettings.statusInstalling");
  if (status === "error") return t("admin.terminalSettings.statusError");
  return t("admin.terminalSettings.statusUninstalled");
});
const tmuxStatusVariant = computed(() => {
  const status = tmuxInstallState.value.status;
  if (status === "installed") return "default";
  if (status === "installing") return "secondary";
  if (status === "error") return "destructive";
  return "outline";
});
const isDirty = computed(() => {
  if (!settings.value) return false;
  return JSON.stringify(settings.value) !== JSON.stringify(form);
});
const terminalDescription = computed(() =>
  configStore.isDockerDeployment
    ? t("admin.terminalSettings.descriptionDocker")
    : t("admin.terminalSettings.description"),
);

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
        toast.error(t("admin.terminalSettings.loadFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.terminalSettings.loadTmuxDescription"),
          ),
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
        toast.success(t("admin.terminalSettings.updated"));
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
        state.status === "installed"
          ? t("admin.terminalSettings.tmuxReady")
          : t("admin.terminalSettings.tmuxInstallStarted"),
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
            <span>{{ t("admin.terminalSettings.title") }}</span>
            <Badge :variant="tmuxStatusVariant">{{ tmuxStatusLabel }}</Badge>
          </CardTitle>
          <CardDescription>{{ terminalDescription }}</CardDescription>
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
        <template v-if="isTmuxInstalled">
          <div
            class="flex items-center justify-between rounded-lg border bg-muted/10 p-4"
          >
            <div class="space-y-1 pr-6">
              <Label
                class="cursor-pointer text-base font-medium"
                @click="form.enabled = !form.enabled"
              >
                {{ t("admin.terminalSettings.enableLabel") }}
              </Label>
              <p class="text-sm text-muted-foreground">
                {{ t("admin.terminalSettings.enableDescription") }}
              </p>
            </div>
            <Switch v-model="form.enabled" :disabled="isSaving" />
          </div>
        </template>

        <div class="grid gap-4 md:grid-cols-1">
          <div class="rounded-lg border bg-muted/20 p-4">
            <div class="mb-2 flex items-center gap-2 text-sm font-medium">
              <TerminalSquare class="h-4 w-4" />
              <span>{{ t("admin.terminalSettings.tmuxEnvironment") }}</span>
            </div>
            <div class="space-y-2 text-sm text-muted-foreground">
              <p>
                <span class="text-foreground">{{
                  t("admin.terminalSettings.statusLabel")
                }}</span>{{ tmuxStatusLabel }}
              </p>
              <p>
                <span class="text-foreground">{{
                  t("admin.terminalSettings.versionLabel")
                }}</span>
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
              <div class="text-sm font-medium">
                {{ t("admin.terminalSettings.installTmuxTitle") }}
              </div>
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
                  ? t("admin.terminalSettings.reinstallTmux")
                  : t("admin.terminalSettings.installTmux")
              }}
            </Button>
          </div>
        </div>

        <template v-else>
          <div class="grid gap-4 md:grid-cols-2">
            <div class="space-y-2">
              <Label for="terminal-max-sessions">
                {{ t("admin.terminalSettings.maxSessions") }}
              </Label>
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
              <Label for="terminal-idle-timeout">
                {{ t("admin.terminalSettings.idleTimeout") }}
              </Label>
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

          <FloatingActionDock
            :active="isDirty"
            inline-class="flex items-center justify-end gap-3"
          >
            <template #inline>
              <Button
                variant="outline"
                :disabled="!isDirty || isSaving"
                @click="resetForm"
              >
                {{ t("admin.terminalSettings.reset") }}
              </Button>
              <Button :disabled="!isDirty || isSaving" @click="saveSettings">
                {{ t("admin.terminalSettings.saveSettings") }}
              </Button>
            </template>
          </FloatingActionDock>
        </template>
      </div>
    </CardContent>
  </Card>
</template>
