<template>
  <Card class="w-full">
    <CardHeader>
      <div class="flex items-start justify-between gap-4">
        <div class="grid gap-1">
          <CardTitle class="flex items-center gap-2">
            ACME.sh
            <Badge :variant="statusBadgeVariant">{{ statusLabel }}</Badge>
          </CardTitle>
          <CardDescription>安装并管理 acme.sh 客户端，供 ACME 证书功能使用。</CardDescription>
        </div>
        <RefreshButton :loading="isFetching" :disabled="isInitializing || isFetching || isSwitchingCa" @click="fetchStatus" />
      </div>
    </CardHeader>

    <CardContent v-if="isInitializing && showInitializingSkeleton" class="grid gap-6">
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="border p-4 rounded-lg">
          <Skeleton class="h-4 w-20 mb-2" />
          <Skeleton class="h-5 w-24" />
          <Skeleton class="h-3 w-40 mt-3" />
        </div>
        <div class="border p-4 rounded-lg md:col-span-2">
          <div class="flex justify-between items-center">
            <Skeleton class="h-4 w-16" />
            <Skeleton class="h-5 w-12" />
          </div>
          <div class="mt-4">
            <Skeleton class="h-3 w-full" />
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else-if="!isInitializing" class="grid gap-6">
      <div class="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div class="border p-4 rounded-lg">
          <div class="text-sm text-muted-foreground mb-2">客户端状态</div>
          <div class="font-medium">{{ statusLabel }}</div>
          <div class="mt-2 text-xs text-muted-foreground font-mono break-all">
            {{ state?.message || '-' }}
          </div>
        </div>

        <div class="border p-4 rounded-lg md:col-span-2">
          <div class="flex justify-between items-center">
            <div class="text-sm text-muted-foreground">资源状态</div>
            <div v-if="isInstalled" :class="['px-2 py-0.5 rounded text-xs font-medium', 'bg-green-100 text-green-700 border border-green-200']">
              已下载
            </div>
            <div v-else :class="['px-2 py-0.5 rounded text-xs font-medium', 'bg-yellow-100 text-yellow-700 border border-yellow-200']">
              <span v-if="!isInstalling">未下载</span>
              <span v-else>下载中</span>
            </div>
          </div>
          <div class="mt-4">
            <Progress v-if="progress < 100" :model-value="progress" />
          </div>
          <div v-if="state?.status === 'error'" class="text-sm bg-destructive/10 text-destructive p-3 rounded-md border border-destructive/20 mt-3 break-all">
            错误：{{ state?.message }}
          </div>
          <div v-else-if="isInstalling" class="text-sm text-muted-foreground mt-3 animate-pulse">
            安装中，请稍候…
          </div>
        </div>
      </div>

      <div v-if="isInstalled" class="rounded-lg border bg-muted/20 p-4">
        <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between sm:gap-4">
          <div class="grid min-w-0 gap-1">
            <div class="text-sm font-medium">证书颁发机构</div>
            <div class="text-xs text-muted-foreground">
              切换后会影响后续证书申请和自动续期，不会立即替换当前已部署证书。
            </div>
          </div>
          <Badge variant="outline" class="shrink-0 self-start whitespace-nowrap">
            {{ currentCertificateAuthorityLabel }}
          </Badge>
        </div>

        <div class="mt-4 grid grid-cols-1 gap-3 md:grid-cols-2">
          <button
            v-for="option in certificateAuthorityOptions"
            :key="option.value"
            type="button"
            :disabled="isSwitchingCa || isDeleting || isFetching || currentCertificateAuthority === option.value"
            :class="[
              'rounded-xl border p-4 text-left transition-colors disabled:cursor-not-allowed disabled:opacity-60',
              currentCertificateAuthority === option.value
                ? 'border-primary bg-primary/5 shadow-sm'
                : 'border-border bg-background hover:border-primary/40 hover:bg-muted/40',
            ]"
            @click="switchCertificateAuthority(option.value)"
          >
            <div class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
              <div class="grid min-w-0 gap-1">
                <div class="text-sm font-medium">{{ option.label }}</div>
                <div class="text-xs text-muted-foreground">{{ option.description }}</div>
              </div>
              <span
                :class="[
                  'self-start shrink-0 whitespace-nowrap rounded-full border px-2 py-0.5 text-[11px] font-medium',
                  currentCertificateAuthority === option.value
                    ? 'border-primary/20 bg-primary/10 text-primary'
                    : 'border-border text-muted-foreground',
                ]"
              >
                {{ currentCertificateAuthority === option.value ? '当前' : '切换' }}
              </span>
            </div>
          </button>
        </div>

        <div class="mt-3 text-xs text-muted-foreground">
          后续签发与自动续期都会以这里的 CA 配置为准。
        </div>
        <div v-if="isSwitchingCa" class="mt-3 text-sm text-muted-foreground animate-pulse">
          正在切换证书颁发机构，请稍候…
        </div>
      </div>

      <div v-if="!isInstalled && !isInstalling" class="rounded-lg border bg-muted/20 p-4">
        <div class="flex items-start justify-between gap-4">
          <div class="grid gap-1">
            <div class="text-sm font-medium">安装配置</div>
            <div class="text-xs text-muted-foreground">点击开始安装后将自动注册 ACME 账号邮箱，无需手动输入。</div>
          </div>
        </div>

        <div class="mt-3">
          <Button class="w-full md:w-auto" :disabled="isStartingInstall" @click="startInstall">
            <span v-if="isStartingInstall" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            开始安装
          </Button>
        </div>
      </div>
    </CardContent>
    <CardContent v-else class="min-h-[220px]" aria-hidden="true" ></CardContent>

    <CardFooter v-if="!isInitializing" class="flex justify-end gap-3 border-t pt-6">
      <template v-if="isInstalling">
        <div class="text-sm text-muted-foreground animate-pulse flex items-center h-10 mr-auto">安装中，请稍候…</div>
        <RefreshButton :loading="isFetching" :disabled="isFetching" @click="fetchStatus" />
      </template>
      <template v-else>
        <ConfirmDangerPopover
          v-if="isInstalled"
          title="确认删除 acme.sh？"
          description="删除后将无法继续申请/续期证书，需要重新安装才能使用。"
          :loading="isDeleting"
          :disabled="isDeleting || isSwitchingCa"
          :on-confirm="uninstall"
          content-class="w-80 text-left"
        >
          <template #trigger>
            <Button variant="destructive" :disabled="isDeleting || isSwitchingCa">删除</Button>
          </template>
        </ConfirmDangerPopover>
      </template>
    </CardFooter>
  </Card>

  <Dialog :open="showSwitchCaDialog" @update:open="handleSwitchCaDialogOpenChange">
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle>切换证书颁发机构</DialogTitle>
        <DialogDescription>
          确认将 ACME 默认 CA 从 {{ currentCertificateAuthorityLabel }} 切换到
          {{ pendingCertificateAuthorityLabel }} 吗？
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-3 text-sm text-muted-foreground">
        <p>
          切换后，后续证书申请和自动续期都会使用新的证书颁发机构。
        </p>
        <p>当前已签发或已部署的证书不会立即被替换。</p>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isSwitchingCa"
          @click="handleSwitchCaDialogOpenChange(false)"
        >
          取消
        </Button>
        <Button :disabled="isSwitchingCa" @click="confirmSwitchCertificateAuthority">
          <span
            v-if="isSwitchingCa"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          确认切换
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue';
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import RefreshButton from '@/components/RefreshButton.vue';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { Badge } from '@/components/ui/badge';
import { toast } from '@admin-shared/utils/toast';
import { AcmeAPI } from '../../lib/api';
import { usePollingResourceStatus } from '@admin-shared/composables/usePollingResourceStatus';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';

type AcmeCertificateAuthority = 'zerossl' | 'letsencrypt';
type AcmeState = {
  status: 'uninstalled' | 'installing' | 'installed' | 'error';
  progress: number;
  message: string;
  certificateAuthority: AcmeCertificateAuthority;
  certificateAuthorityUpdatedAt?: string;
};

const certificateAuthorityOptions: Array<{
  value: AcmeCertificateAuthority;
  label: string;
  description: string;
}> = [
  {
    value: 'letsencrypt',
    label: "Let's Encrypt",
    description: '更常见的免费 CA，适合大多数通用场景。',
  },
  {
    value: 'zerossl',
    label: 'ZeroSSL',
    description: '与 acme.sh 默认流程兼容，适合继续沿用现有行为。',
  },
];

const state = ref<AcmeState | null>(null);
const showSwitchCaDialog = ref(false);
const pendingCertificateAuthority = ref<AcmeCertificateAuthority | null>(null);
const { isPending: isFetching, run: runFetchStatus } = useAsyncAction();
const { isPending: isStartingInstall, run: runStartInstall } = useAsyncAction({
  onError: async (error) => {
    toast.error(extractErrorMessage(error, '安装失败'));
    await fetchStatus();
  },
});
const { isPending: isDeleting, run: runUninstall } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '删除失败'));
    void fetchStatus();
  },
});
const { isPending: isSwitchingCa, run: runSwitchCa } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '切换证书颁发机构失败'));
    void fetchStatus();
  },
});

const isInstalling = computed(() => state.value?.status === 'installing');
const isInstalled = computed(() => state.value?.status === 'installed');
const currentCertificateAuthority = computed<AcmeCertificateAuthority>(
  () => state.value?.certificateAuthority || 'zerossl',
);
const currentCertificateAuthorityLabel = computed(() => {
  return (
    certificateAuthorityOptions.find(
      (option) => option.value === currentCertificateAuthority.value,
    )?.label || 'ZeroSSL'
  );
});
const pendingCertificateAuthorityLabel = computed(() => {
  return (
    certificateAuthorityOptions.find(
      (option) => option.value === pendingCertificateAuthority.value,
    )?.label || '-'
  );
});

const progress = computed(() => {
  const v = state.value?.progress ?? 0;
  return Math.max(0, Math.min(100, Number.isFinite(v) ? v : 0));
});

const statusLabel = computed(() => {
  const s = state.value?.status;
  if (!s) return '未知';
  if (s === 'installed') return '已安装';
  if (s === 'installing') return '安装中';
  if (s === 'error') return '错误';
  return '未安装';
});

const statusBadgeVariant = computed(() => {
  const s = state.value?.status;
  if (s === 'installed') return 'default';
  if (s === 'installing') return 'secondary';
  if (s === 'error') return 'destructive';
  return 'outline';
});

const { isInitializing, refresh: fetchStatus } = usePollingResourceStatus<AcmeState | null>({
  fetcher: async () => {
    const data = await runFetchStatus(() => AcmeAPI.status());
    return data ?? state.value;
  },
  onData: (data) => {
    state.value = data;
  },
  isDownloading: (data) => data?.status === 'installing',
});
const showInitializingSkeleton = useDelayedLoading(isInitializing);

async function startInstall() {
  if (isInstalling.value) return;
  await runStartInstall(
    () => AcmeAPI.init(),
    {
      onSuccess: async () => {
        toast.success('已开始安装');
        await fetchStatus();
      },
    },
  );
}

async function uninstall() {
  if (!isInstalled.value) return;
  await runUninstall(async () => {
    await AcmeAPI.uninstall();
    toast.success('已删除 acme.sh');
    await fetchStatus();
  });
}

function switchCertificateAuthority(next: AcmeCertificateAuthority) {
  if (!isInstalled.value) return;
  if (currentCertificateAuthority.value === next) return;
  pendingCertificateAuthority.value = next;
  showSwitchCaDialog.value = true;
}

async function confirmSwitchCertificateAuthority() {
  const next = pendingCertificateAuthority.value;
  if (!next) return;
  await runSwitchCa(async () => {
    await AcmeAPI.updateClientSettings({ certificateAuthority: next });
    toast.success(
      `已切换到 ${
        certificateAuthorityOptions.find((option) => option.value === next)?.label || next
      }`,
    );
    showSwitchCaDialog.value = false;
    pendingCertificateAuthority.value = null;
    await fetchStatus();
  });
}

function handleSwitchCaDialogOpenChange(open: boolean) {
  showSwitchCaDialog.value = open;
  if (!open && !isSwitchingCa.value) {
    pendingCertificateAuthority.value = null;
  }
}

</script>
