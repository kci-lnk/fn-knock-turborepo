<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { Card, CardContent } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { useUpdateStore } from '../store/update';
import {
  Github,
  RefreshCw,
  Rocket,
  ArrowRight,
  CheckCircle2,
  Sparkles,
  Terminal,
  MonitorUp,
  AlertCircle
} from 'lucide-vue-next';

const updateStore = useUpdateStore();
const showInstallingOverlay = ref(false);

const status = computed(() => updateStore.status);
const releaseNotesHtml = computed(() => {
  const raw = status.value?.latest?.release_notes || '暂无更新说明';
  let html = raw
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');

  html = html.replace(
    /\[([^\]]+)\]\((https?:\/\/[^\s)]+)\)/g,
    '<a href="$2" target="_blank" rel="noopener noreferrer" class="underline underline-offset-4 decoration-primary/50 text-primary hover:decoration-primary hover:opacity-80 transition-all font-medium">$1</a>'
  );

  return html;
});

const downloadState = computed(() => status.value?.download.status ?? 'idle');

const progressValue = computed(() => status.value?.download.percent ?? 0);
const progressText = computed(() => {
  const current = status.value;
  if (!current) return '';
  const bytes = current.download.downloadedBytes;
  const total = current.download.totalBytes;
  if (!total || total <= 0) return `${bytes} B`;
  const toMB = (v: number) => (v / (1024 * 1024)).toFixed(2);
  return `${toMB(bytes)} MB / ${toMB(total)} MB`;
});

const canTriggerOneClick = computed(() => {
  return !updateStore.isChecking
    && !updateStore.isTriggeringDownload
    && !updateStore.isTriggeringInstall
    && downloadState.value !== 'downloading'
    && downloadState.value !== 'verifying'
    && downloadState.value !== 'installing';
});

const oneClickLabel = computed(() => {
  if (downloadState.value === 'downloading' || downloadState.value === 'verifying' || downloadState.value === 'installing') {
    return '更新进行中...';
  }
  if (updateStore.canInstall) return '一键更新 (安装并重启)';
  return '一键更新';
});

const showOneClickUpdateButton = computed(() => Boolean(status.value?.hasUpdate));

const isDownloadingOrVerifying = computed(() => {
  return downloadState.value === 'downloading' || downloadState.value === 'verifying';
});

const isUpdateModalVisible = computed(() => {
  return isDownloadingOrVerifying.value || showInstallingOverlay.value || downloadState.value === 'installing';
});

const modalTitle = computed(() => {
  if (downloadState.value === 'downloading') return '正在下载更新文件';
  if (downloadState.value === 'verifying') return '正在校验文件完整性';
  return '正在执行更新';
});

const modalDescription = computed(() => {
  if (isDownloadingOrVerifying.value) {
    return '请保持网络畅通，耐心等待下载完成...';
  }
  return '系统正在安装更新，请勿关闭此页面。<br>稍后桌面会重新出现“敲门 knock”应用。';
});

const checkNow = async () => {
  await updateStore.checkNow(true);
};

const openGithub = () => {
  const githubUrl = status.value?.githubUrl;
  if (!githubUrl) return;
  window.open(githubUrl, '_blank', 'noopener,noreferrer');
};

const startInstallFlow = async () => {
  showInstallingOverlay.value = true;
  await new Promise((resolve) => window.setTimeout(resolve, 250));
  const started = await updateStore.startInstall();
  if (!started) {
    showInstallingOverlay.value = false;
  }
};

const oneClickUpdate = async () => {
  if (updateStore.canInstall) {
    await startInstallFlow();
    return;
  }
  await updateStore.checkAndDownload();
};

onMounted(() => {
  void updateStore.initialize();
});
</script>

<template>
  <div class="mx-auto space-y-6">

    <Card class="border-border/50 shadow-sm overflow-hidden">
      <CardContent class="space-y-8">

        <div class="flex items-center justify-between px-1">
          <div>
            <h2 class="text-2xl font-semibold tracking-tight">系统更新</h2>
            <p class="text-sm text-muted-foreground mt-1">管理 Fn-Knock 的版本与升级</p>
          </div>
          <Button variant="ghost" size="icon" class="rounded-full hover:bg-muted" :disabled="!status?.githubUrl"
            title="打开 GitHub 项目页" @click="openGithub">
            <Github class="h-5 w-5" />
          </Button>
        </div>

        <div class="flex items-center justify-center py-6 px-4 bg-muted/30 rounded-2xl border border-border/50">
          <div class="flex flex-col items-center flex-1 space-y-1">
            <span class="text-sm font-medium text-muted-foreground">当前版本</span>
            <span class="text-2xl text-gray-600 font-bold font-mono tracking-tight text-foreground">
              {{ status?.localVersion || '...' }}
            </span>
          </div>

          <div class="px-4 md:px-8 text-muted-foreground/40">
            <ArrowRight class="w-6 h-6" />
          </div>

          <div class="flex flex-col items-center flex-1 space-y-1">
            <span class="text-sm font-medium text-muted-foreground">最新版本</span>
            <span class="text-2xl font-bold font-mono tracking-tight"
              :class="status?.hasUpdate ? 'text-primary' : 'text-foreground'">
              {{ status?.latest?.version || '...' }}
            </span>
          </div>
        </div>

        <div class="flex flex-col sm:flex-row items-stretch sm:items-center justify-between gap-6 bg-background">
          <div class="flex items-center gap-3 w-full sm:w-auto">
            <div class="flex items-center justify-center w-10 h-10 rounded-full"
              :class="status?.hasUpdate ? 'bg-primary/10 text-primary' : 'bg-muted text-muted-foreground'">
              <Sparkles v-if="status?.hasUpdate" class="w-5 h-5" />
              <CheckCircle2 v-else-if="status?.updateEnabled" class="w-5 h-5" />
              <AlertCircle v-else class="w-5 h-5" />
            </div>
            <div class="space-y-0.5">
              <p class="text-sm font-medium">
                {{
                  status?.hasUpdate ? '发现新版本，建议立即更新' :
                    status?.updateEnabled ? '当前已是最新版本' :
                      '更新功能暂未启用'
                }}
              </p>
              <p class="text-xs text-muted-foreground">
                {{ status?.hasUpdate ? '一键获取最新特性与修复。' : '感谢使用最新版本。' }}
              </p>
            </div>
          </div>

          <div class="flex items-center gap-2 sm:gap-3 w-full sm:w-auto">
            <Button
              variant="outline"
              class="flex-1 min-w-0 sm:flex-none sm:w-auto"
              :disabled="updateStore.isChecking"
              @click="checkNow"
            >
              <RefreshCw class="mr-2 h-4 w-4" :class="updateStore.isChecking ? 'animate-spin' : ''" />
              检查更新
            </Button>
            <Button
              v-if="showOneClickUpdateButton"
              class="flex-1 min-w-0 sm:flex-none sm:w-auto shadow-sm"
              :disabled="!canTriggerOneClick"
              :variant="updateStore.canInstall ? 'destructive' : 'default'"
              @click="oneClickUpdate"
            >
              <Rocket class="mr-2 h-4 w-4" />
              <span class="sm:hidden">{{ updateStore.canInstall ? '安装重启' : oneClickLabel }}</span>
              <span class="hidden sm:inline">{{ oneClickLabel }}</span>
            </Button>
          </div>
        </div>

        <Alert v-if="status?.check.error" variant="destructive" class="rounded-xl">
          <AlertCircle class="w-4 h-4" />
          <AlertTitle>更新检查失败</AlertTitle>
          <AlertDescription>{{ status.check.error }}</AlertDescription>
        </Alert>

        <Alert v-if="status?.download.error" variant="destructive" class="rounded-xl">
          <AlertCircle class="w-4 h-4" />
          <AlertTitle>更新失败</AlertTitle>
          <AlertDescription>{{ status.download.error }}</AlertDescription>
        </Alert>

        <div v-if="status?.latest?.release_notes" class="pt-4 border-t border-border/40">
          <h3 class="text-sm font-medium flex items-center gap-2 mb-4 text-foreground">
            <Terminal class="w-4 h-4 text-muted-foreground" />
            版本更新说明
          </h3>
          <div class="p-5 rounded-2xl bg-muted/30 border border-border/40">
            <div
              class="text-sm text-muted-foreground font-sans whitespace-pre-wrap break-words leading-relaxed"
              v-html="releaseNotesHtml"
            ></div>
          </div>
        </div>

      </CardContent>
    </Card>

    <div v-if="isUpdateModalVisible"
      class="fixed inset-0 z-[120] flex items-center justify-center bg-background/80 backdrop-blur-md px-4 transition-all duration-300">
      <div
        class="w-full max-w-sm rounded-2xl border border-border/50 bg-background/95 p-8 shadow-2xl flex flex-col items-center text-center space-y-6">
        
        <div class="w-16 h-16 rounded-full bg-primary/10 flex items-center justify-center">
          <RefreshCw v-if="isDownloadingOrVerifying" class="w-8 h-8 text-primary animate-spin" />
          <MonitorUp v-else class="w-8 h-8 text-primary animate-pulse" />
        </div>
        
        <div class="space-y-2">
          <h3 class="text-lg font-semibold tracking-tight">{{ modalTitle }}</h3>
          <p class="text-sm text-muted-foreground leading-relaxed" v-html="modalDescription"></p>
        </div>

        <div v-if="isDownloadingOrVerifying" class="w-full space-y-3">
          <Progress :model-value="progressValue" class="h-2 w-full" />
          <div class="flex justify-between text-xs text-muted-foreground font-mono">
            <span>{{ progressText }}</span>
            <span class="text-primary font-bold">{{ progressValue }}%</span>
          </div>
        </div>

      </div>
    </div>
  </div>
</template>
