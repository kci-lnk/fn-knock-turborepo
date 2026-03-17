<template>
  <Card v-if="isLoading && showLoadingSkeleton && !sslStatus">
    <CardHeader>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <CardTitle>SSL 证书</CardTitle>
        </div>
      </div>
      <CardDescription>加载中...</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-4">
      <div class="rounded-lg border bg-muted/30 p-4 grid gap-3">
        <div class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]">
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-56" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-64" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-40" />
          <Skeleton class="h-4 w-16" />
          <Skeleton class="h-4 w-48" />
        </div>
      </div>
    </CardContent>
    <CardFooter class="flex justify-end">
      <Skeleton class="h-10 w-28" />
    </CardFooter>
  </Card>

  <Card v-else-if="isLoading && !sslStatus" class="min-h-[260px]" aria-hidden="true" ></Card>

  <Card v-else-if="sslStatus?.enabled">
    <CardHeader>
      <div class="flex items-center justify-between">
        <div class="flex items-center gap-3">
          <CardTitle>SSL 证书</CardTitle>
          <Badge variant="default" class="bg-green-600 hover:bg-green-600">
            <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-6 w-6" viewBox="0 0 23 23" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/></svg>
            已启用
          </Badge>
        </div>
      </div>
      <CardDescription>SSL 证书已配置，HTTPS 已启用。</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-4">
      <div v-if="sslStatus.certInfo" class="rounded-lg border bg-muted/30 p-4 grid gap-3">
        <div class="grid grid-cols-[88px_minmax(0,1fr)] gap-y-3 text-sm sm:grid-cols-[100px_minmax(0,1fr)]">
          <span class="text-muted-foreground font-medium">签发者</span>
          <span class="min-w-0 font-mono text-xs break-all">{{ formatDN(sslStatus.certInfo.issuer) }}</span>
          
          <span class="text-muted-foreground font-medium">签发给</span>
          <span class="min-w-0 font-mono text-xs break-all">{{ formatDN(sslStatus.certInfo.subject) }}</span>
          
          <span class="text-muted-foreground font-medium">有效期</span>
          <span class="min-w-0 text-xs">
            <span>{{ formatDate(sslStatus.certInfo.validFrom) }}</span>
            <span class="mx-1 text-muted-foreground">至</span>
            <span :class="isExpired ? 'text-destructive font-semibold' : ''">{{ formatDate(sslStatus.certInfo.validTo) }}</span>
            <Badge v-if="isExpired" variant="destructive" class="ml-2 text-[10px]">已过期</Badge>
            <Badge v-else-if="isExpiringSoon" variant="outline" class="ml-2 text-[10px] border-yellow-500 text-yellow-600">即将过期</Badge>
          </span>

          <span class="text-muted-foreground font-medium">域名</span>
          <div class="min-w-0 flex flex-wrap gap-1.5">
            <Badge v-for="dns in sslStatus.certInfo.dnsNames" :key="dns" variant="secondary" class="font-mono text-xs">{{ dns }}</Badge>
            <span v-if="!sslStatus.certInfo.dnsNames.length" class="text-xs text-muted-foreground">无</span>
          </div>

          <span class="text-muted-foreground font-medium">序列号</span>
          <span class="min-w-0 font-mono text-xs text-muted-foreground break-all">{{ sslStatus.certInfo.serialNumber }}</span>
        </div>
      </div>
    </CardContent>
    <CardFooter class="flex flex-wrap justify-end gap-2">
      <ConfirmDangerPopover
        title="确认清除 SSL 证书"
        description="清除后将禁用 HTTPS，所有连接将回退到 HTTP。"
        confirm-text="确认清除"
        :loading="isClearing"
        :disabled="isClearing"
        :on-confirm="handleClear"
      >
        <template #trigger>
          <Button variant="destructive" size="sm" :disabled="isClearing">清除证书</Button>
        </template>
      </ConfirmDangerPopover>
      <Button variant="outline" size="sm" @click="showEditDialog = true">修改证书</Button>
    </CardFooter>
  </Card>

  <Card v-else>
    <CardHeader>
      <div class="flex items-center gap-3">
        <CardTitle>SSL 证书</CardTitle>
        <Badge variant="secondary" class="text-muted-foreground">
          <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>
          未启用
        </Badge>
      </div>
      <CardDescription>上传或粘贴您的 SSL 证书和私钥以启用 HTTPS。</CardDescription>
    </CardHeader>
    <CardContent class="grid gap-6">
      <CertForm
        v-model:cert="formData.cert"
        v-model:sslKey="formData.key"
        :share-name="sslSharedFiles.shareName"
        :shared-files="sslSharedFiles.files"
        :shared-files-available="sslSharedFiles.available"
        :shared-files-loading="isLoadingSharedFiles"
        :shared-files-error="sharedFilesError"
        :shared-file-selecting="isReadingSharedFile"
        @request-shared-files="handleSharedFilesRequest"
        @select-shared-file="handleCreateSharedFileSelect"
      />
      <Alert v-if="errorMessage" variant="destructive">
        <AlertTitle>证书验证失败</AlertTitle>
        <AlertDescription>{{ errorMessage }}</AlertDescription>
      </Alert>
    </CardContent>
    <CardFooter class="flex flex-wrap justify-end">
      <Button @click="handleSave" :disabled="isSaving || !formData.cert || !formData.key">
        <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
        保存证书
      </Button>
    </CardFooter>
  </Card>

  <component
    :is="isMobileViewport ? Sheet : Dialog"
    :open="showEditDialog"
    @update:open="showEditDialog = $event"
  >
    <component
      :is="isMobileViewport ? SheetContent : DialogContent"
      v-bind="isMobileViewport ? { side: 'bottom' } : {}"
      :class="isMobileViewport
        ? 'flex h-[92vh] flex-col gap-0 rounded-t-[10px] border-x-0 border-b-0 bg-background px-0 pb-0'
        : 'flex max-h-[90vh] flex-col border-border/60 bg-background sm:max-w-[600px]'"
    >
      <component
        :is="isMobileViewport ? SheetHeader : DialogHeader"
        class="px-6 pb-0 pt-6"
      >
        <component :is="isMobileViewport ? SheetTitle : DialogTitle">修改 SSL 证书</component>
        <component :is="isMobileViewport ? SheetDescription : DialogDescription">
          上传或粘贴新的 SSL 证书和私钥来替换当前证书。
        </component>
      </component>
      <div class="grid min-h-0 flex-1 gap-6 overflow-y-auto px-6 py-4">
        <CertForm
          v-model:cert="editFormData.cert"
          v-model:sslKey="editFormData.key"
          :share-name="sslSharedFiles.shareName"
          :shared-files="sslSharedFiles.files"
          :shared-files-available="sslSharedFiles.available"
          :shared-files-loading="isLoadingSharedFiles"
          :shared-files-error="sharedFilesError"
          :shared-file-selecting="isReadingSharedFile"
          @request-shared-files="handleSharedFilesRequest"
          @select-shared-file="handleEditSharedFileSelect"
        />
        <Alert v-if="editErrorMessage" variant="destructive">
          <AlertTitle>证书验证失败</AlertTitle>
          <AlertDescription>{{ editErrorMessage }}</AlertDescription>
        </Alert>
      </div>
      <component
        :is="isMobileViewport ? SheetFooter : DialogFooter"
        class="border-t border-border/50 bg-background px-6 py-4"
      >
        <Button variant="outline" @click="showEditDialog = false">取消</Button>
        <Button @click="handleEdit" :disabled="isSaving || !editFormData.cert || !editFormData.key">
          <span v-if="isSaving" class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          保存修改
        </Button>
      </component>
    </component>
  </component>
</template>

<script setup lang="ts">
import { ref, computed, onBeforeUnmount, onMounted } from 'vue';
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Badge } from '@/components/ui/badge';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { Sheet, SheetContent, SheetDescription, SheetFooter, SheetHeader, SheetTitle } from '@/components/ui/sheet';
import { Skeleton } from '@/components/ui/skeleton';
import CertForm from '@admin-shared/components/ssl/CertForm.vue';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { ConfigAPI } from '../../lib/api';
import type { SSLSharedFilesPayload, SSLStatus } from '../../types';
import { toast } from '@admin-shared/utils/toast';

const sslStatus = ref<SSLStatus | null>(null);
const errorMessage = ref('');
const editErrorMessage = ref('');
const showEditDialog = ref(false);
const sharedFilesError = ref('');
const isMobileViewport = ref(false);

const formData = ref({ cert: '', key: '' });
const editFormData = ref({ cert: '', key: '' });
const defaultSSLSharedFiles: SSLSharedFilesPayload = {
  shareName: 'fn-knock',
  available: false,
  files: [],
};
const sslSharedFiles = ref<SSLSharedFilesPayload>(defaultSSLSharedFiles);
const hasLoadedSharedFiles = ref(false);
const saveMode = ref<'create' | 'edit'>('create');
const { isPending: isSaving, run: runSaveSSL } = useAsyncAction({
  onError: (error) => {
    const msg = extractErrorMessage(error, '保存失败，请检查证书和私钥格式。');
    if (saveMode.value === 'edit') {
      editErrorMessage.value = msg;
    } else {
      errorMessage.value = msg;
    }
    toast.error(msg);
  },
});
const { isPending: isClearing, run: runClearSSL } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '证书清除失败'));
  },
});
const { isPending: isLoading, run: runLoadSSLStatus } = useAsyncAction({
  onError: (error) => {
    console.error('Failed to load SSL status:', error);
  },
});
const { isPending: isLoadingSharedFiles, run: runLoadSharedFiles } = useAsyncAction({
  onError: (error) => {
    const message = extractErrorMessage(error, '读取飞牛共享目录失败');
    sharedFilesError.value = message;
    toast.error(message);
  },
});
const { isPending: isReadingSharedFile, run: runReadSharedFile } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '读取共享文件失败'));
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);

let viewportQuery: MediaQueryList | null = null;
let viewportQueryListener: ((event: MediaQueryListEvent) => void) | null = null;

const isExpired = computed(() => {
  if (!sslStatus.value?.certInfo?.validTo) return false;
  return new Date(sslStatus.value.certInfo.validTo) < new Date();
});

const isExpiringSoon = computed(() => {
  if (!sslStatus.value?.certInfo?.validTo) return false;
  const validTo = new Date(sslStatus.value.certInfo.validTo);
  const now = new Date();
  const thirtyDays = 30 * 24 * 60 * 60 * 1000;
  return validTo > now && (validTo.getTime() - now.getTime()) < thirtyDays;
});

onMounted(() => {
  if (typeof window !== 'undefined') {
    viewportQuery = window.matchMedia('(max-width: 768px)');
    isMobileViewport.value = viewportQuery.matches;

    viewportQueryListener = (event: MediaQueryListEvent) => {
      isMobileViewport.value = event.matches;
    };

    if (typeof viewportQuery.addEventListener === 'function') {
      viewportQuery.addEventListener('change', viewportQueryListener);
    } else {
      viewportQuery.addListener(viewportQueryListener);
    }
  }

  loadSSLStatus();
});

onBeforeUnmount(() => {
  if (!viewportQuery || !viewportQueryListener) {
    return;
  }

  if (typeof viewportQuery.removeEventListener === 'function') {
    viewportQuery.removeEventListener('change', viewportQueryListener);
    return;
  }

  viewportQuery.removeListener(viewportQueryListener);
});

async function loadSSLStatus() {
  await runLoadSSLStatus(async () => {
    sslStatus.value = await ConfigAPI.getSSLStatus();
  });
}

async function handleSave() {
  saveMode.value = 'create';
  errorMessage.value = '';
  await runSaveSSL(async () => {
    await ConfigAPI.setSSL(formData.value);
    formData.value = { cert: '', key: '' };
    await loadSSLStatus();
    toast.success('证书启用成功');
  });
}

async function handleEdit() {
  saveMode.value = 'edit';
  editErrorMessage.value = '';
  await runSaveSSL(async () => {
    await ConfigAPI.setSSL(editFormData.value);
    editFormData.value = { cert: '', key: '' };
    showEditDialog.value = false;
    await loadSSLStatus();
    toast.success('证书修改成功');
  });
}

async function handleClear() {
  await runClearSSL(async () => {
    await ConfigAPI.deleteSSL();
    await loadSSLStatus();
    toast.success('证书清除成功');
  });
}

async function loadSharedFiles(force = false) {
  if (hasLoadedSharedFiles.value && !force) {
    return;
  }

  sharedFilesError.value = '';
  const nextFiles = await runLoadSharedFiles(async () => ConfigAPI.getSSLSharedFiles());
  if (!nextFiles) {
    return;
  }

  sslSharedFiles.value = nextFiles;
  hasLoadedSharedFiles.value = true;
}

async function handleSharedFilesRequest(payload: { field: 'cert' | 'sslKey'; force?: boolean }) {
  await loadSharedFiles(Boolean(payload.force));
}

async function applySharedFileSelection(
  target: { cert: string; key: string },
  payload: { field: 'cert' | 'sslKey'; relativePath: string },
) {
  const result = await runReadSharedFile(async () => ConfigAPI.readSSLSharedFile(payload.relativePath));
  if (!result) {
    return;
  }

  if (payload.field === 'cert') {
    target.cert = result.content;
  } else {
    target.key = result.content;
  }

  const label = payload.field === 'cert' ? '证书' : '私钥';
  toast.success(`已从飞牛目录载入${label}文件：${result.file.name}`);
}

async function handleCreateSharedFileSelect(payload: { field: 'cert' | 'sslKey'; relativePath: string }) {
  await applySharedFileSelection(formData.value, payload);
}

async function handleEditSharedFileSelect(payload: { field: 'cert' | 'sslKey'; relativePath: string }) {
  await applySharedFileSelection(editFormData.value, payload);
}

function formatDN(dn: string): string {
  return dn.replace(/\n/g, ', ');
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;
  return date.toLocaleDateString('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  });
}
</script>
