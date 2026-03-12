<template>
  <div class="grid gap-4">
    <Card v-if="isInitializing && showInitializingSkeleton">
      <CardHeader>
        <CardTitle>根证书管理</CardTitle>
        <CardDescription>使用固定主体信息生成自签根证书，保存于后端并可下载安装。</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4">
        <div class="rounded-lg border bg-muted/30 p-4 grid gap-3 text-sm">
          <div class="grid grid-cols-[110px_1fr] gap-y-2">
            <Skeleton class="h-4 w-12" />
            <Skeleton class="h-4 w-64" />
            <Skeleton class="h-4 w-12" />
            <Skeleton class="h-4 w-64" />
            <Skeleton class="h-4 w-12" />
            <Skeleton class="h-4 w-40" />
            <Skeleton class="h-4 w-12" />
            <Skeleton class="h-4 w-48" />
          </div>
        </div>
      </CardContent>
      <CardFooter class="flex gap-2">
        <Skeleton class="h-10 w-28" />
        <Skeleton class="h-10 w-28" />
      </CardFooter>
    </Card>

    <Card v-else-if="!isInitializing">
      <CardHeader>
        <CardTitle>根证书管理</CardTitle>
        <CardDescription>使用固定主体信息生成自签根证书，保存于后端并可下载安装。</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4">
        <Alert v-if="!hasRootCA" variant="destructive">
          <AlertTitle>尚未初始化根证书</AlertTitle>
          <AlertDescription>点击下方按钮生成随机密钥对与自签根证书。</AlertDescription>
        </Alert>
        <div v-else class="rounded-lg border bg-muted/30 p-4 grid gap-3 text-sm">
          <Badge variant="default" class="bg-green-600 hover:bg-green-600">
            <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-3 w-3" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
              <path d="m9 12 2 2 4-4" />
            </svg>
            根证书
          </Badge>
          <div class="grid grid-cols-[110px_1fr] gap-y-2">
            <span class="text-muted-foreground font-medium">主体</span>
            <span class="font-mono text-xs break-all">{{ caInfo?.subject }}</span>
            <span class="text-muted-foreground font-medium">签发者</span>
            <span class="font-mono text-xs break-all">{{ caInfo?.issuer }}</span>
            <span class="text-muted-foreground font-medium">有效期</span>
            <span class="text-xs">
              <span>{{ caInfo ? formatDate(caInfo.validFrom) : '' }}</span>
              <span class="mx-1 text-muted-foreground">至</span>
              <span>{{ caInfo ? formatDate(caInfo.validTo) : '' }}</span>
            </span>
            <span class="text-muted-foreground font-medium">序列号</span>
            <span class="font-mono text-xs break-all text-muted-foreground">{{ caInfo?.serialNumber }}</span>
          </div>
        </div>
      </CardContent>
      <CardFooter class="flex gap-2">
        <Button v-if="!hasRootCA" @click="generateRootCA" :disabled="isBusy">
          <span v-if="isBusy"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          初始化根证书
        </Button>
        <template v-else>

          <div class="inline-flex items-stretch">
            <ButtonGroup>
              <Button variant="outline" @click="downloadCA" :disabled="isBusy || isDownloading">下载根证书</Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button variant="outline" size="icon" aria-label="更多操作" :disabled="isBusy || isDownloading">
                    <MoreHorizontal class="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" class="w-52">
                  <DropdownMenuGroup>
                    <DropdownMenuItem @click="openRegenFirstConfirm">
                      <RefreshCw class="mr-2 h-4 w-4" />
                      重新生成
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuItem variant="destructive" @click="openFirstConfirm">
                      <Trash2 class="mr-2 h-4 w-4" />
                      清除根证书
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                </DropdownMenuContent>
              </DropdownMenu>
            </ButtonGroup>
          </div>
        </template>
      </CardFooter>
    </Card>
    <Card v-else class="min-h-[260px]" aria-hidden="true" ></Card>

    <Card v-if="!isInitializing">
      <CardHeader>
        <CardTitle>域名与 IP 列表</CardTitle>
        <CardDescription>将为以下主机名或 IP 签发 20 年有效期的服务器证书。</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-3">
        <div class="flex gap-2">
          <Input v-model="newHost" placeholder="输入域名或IP，例如: example.com 或 10.0.0.1" @keydown.enter.prevent="addHost" />
          <Button @click="addHost" :disabled="!pendingHosts.length">添加</Button>
        </div>
        <div class="rounded-md border overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-[60px]">#</TableHead>
                <TableHead>主机名 / IP</TableHead>
                <TableHead class="w-[120px]">类型</TableHead>
                <TableHead class="w-[100px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="(h, idx) in hosts" :key="h + idx">
                <TableCell>{{ idx + 1 }}</TableCell>
                <TableCell class="font-mono text-xs">{{ h }}</TableCell>
                <TableCell>
                  <Badge variant="secondary">{{ isIP(h) ? 'IP' : 'DNS' }}</Badge>
                </TableCell>
                <TableCell>
                  <ConfirmDangerPopover
                    title="确认移除主机"
                    description="将从列表中移除此主机名或 IP，是否继续？"
                    confirm-text="确认移除"
                    :loading="isRemoving && removingHost === h"
                    :disabled="isRemoving && removingHost === h"
                    :on-confirm="() => confirmRemoveHost(h)"
                    content-class="w-72 text-left"
                  >
                    <template #trigger>
                      <Button size="sm" variant="ghost" :disabled="isRemoving && removingHost === h">移除</Button>
                    </template>
                  </ConfirmDangerPopover>
                </TableCell>
              </TableRow>
              <TableRow v-if="!hosts.length">
                <TableCell colspan="4" class="text-center text-muted-foreground">暂无条目</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
      <CardFooter class="flex justify-end">
        <Button @click="issueAndInstall" :disabled="!hasRootCA || !hosts.length || isBusy">
          <span v-if="isBusy"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          一键部署
        </Button>
        <Button variant="outline" class="ml-2" @click="downloadServer" :disabled="isBusy || isDownloading">下载证书</Button>
      </CardFooter>
    </Card>
    <Card v-else-if="showInitializingSkeleton">
      <CardHeader>
        <CardTitle>域名与 IP 列表</CardTitle>
        <CardDescription>将为以下主机名或 IP 签发 20 年有效期的服务器证书。</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-3">
        <div class="flex gap-2">
          <Skeleton class="h-9 w-80" />
          <Skeleton class="h-9 w-20" />
        </div>
        <div class="rounded-md border overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-[60px]">#</TableHead>
                <TableHead>主机名 / IP</TableHead>
                <TableHead class="w-[120px]">类型</TableHead>
                <TableHead class="w-[100px]">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-for="n in 5" :key="n">
                <TableCell><Skeleton class="h-4 w-4" /></TableCell>
                <TableCell><Skeleton class="h-4 w-64" /></TableCell>
                <TableCell><Skeleton class="h-4 w-10" /></TableCell>
                <TableCell><Skeleton class="h-8 w-16 rounded-md" /></TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
      <CardFooter class="flex justify-end">
        <Skeleton class="h-10 w-28" />
        <Skeleton class="h-10 w-28 ml-2" />
      </CardFooter>
    </Card>
    <Card v-else class="min-h-[320px]" aria-hidden="true" ></Card>
    <Dialog :open="showFirstConfirm" @update:open="showFirstConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>确认清除根证书</DialogTitle>
          <DialogDescription>清除后将删除后端保存的根密钥与根证书，无法继续签发新证书。</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showFirstConfirm = false">取消</Button>
          <Button variant="destructive" @click="confirmFirst" :disabled="isClearing">
            <span v-if="isClearing"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            下一步
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showSecondConfirm" @update:open="showSecondConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>二次确认</DialogTitle>
          <DialogDescription>该操作不可恢复，将删除根证书及密钥，且依赖此 CA 的证书将失效，是否继续？</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showSecondConfirm = false">取消</Button>
          <Button variant="destructive" @click="confirmFinalClear" :disabled="isClearing">
            <span v-if="isClearing"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            确认清除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showRegenFirstConfirm" @update:open="showRegenFirstConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>确认重新生成根证书</DialogTitle>
          <DialogDescription>重新生成将覆盖现有根证书与密钥。由该 CA 签发的服务器证书将不再受信任。</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showRegenFirstConfirm = false">取消</Button>
          <Button variant="destructive" @click="confirmRegenFirst" :disabled="isRegenerating">
            <span v-if="isRegenerating"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            下一步
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showRegenSecondConfirm" @update:open="showRegenSecondConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>二次确认</DialogTitle>
          <DialogDescription>该操作不可恢复，将重新生成根密钥与根证书，并替换后端存储。是否继续？</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showRegenSecondConfirm = false">取消</Button>
          <Button variant="destructive" @click="confirmFinalRegen" :disabled="isRegenerating">
            <span v-if="isRegenerating"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            确认重新生成
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card';
import { Button } from '@/components/ui/button';
import { Alert, AlertTitle, AlertDescription } from '@/components/ui/alert';
import { Input } from '@/components/ui/input';
import { Badge } from '@/components/ui/badge';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, DropdownMenuGroup, DropdownMenuSeparator } from '@/components/ui/dropdown-menu';
import { ButtonGroup } from '@/components/ui/button-group'
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { MoreHorizontal, RefreshCw, Trash2 } from 'lucide-vue-next';
import { ConfigAPI } from '../../lib/api';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { downloadBlob } from '@admin-shared/utils/downloadBlob';

const newHost = ref('');
const hosts = ref<string[]>([]);
const parseHosts = (value: string) =>
  value
    .split(/[，,]/g)
    .map((item) => item.trim())
    .filter((item) => item.length > 0);
const pendingHosts = computed(() => {
  const entries = parseHosts(newHost.value);
  return [...new Set(entries)];
});

const hasRootCA = ref(false);
const caInfo = ref<{ subject: string; issuer: string; validFrom: string; validTo: string; serialNumber: string } | null>(null);
const isInitializing = ref(true);
const showInitializingSkeleton = useDelayedLoading(isInitializing);
const removingHost = ref<string | null>(null);
const showFirstConfirm = ref(false);
const showSecondConfirm = ref(false);
const showRegenFirstConfirm = ref(false);
const showRegenSecondConfirm = ref(false);
const { isPending: isBusy, run: runBusyAction } = useAsyncAction();
const { isPending: isRemoving, run: runRemoveHostAction } = useAsyncAction();
const { isPending: isClearing, run: runClearRootCA } = useAsyncAction({
  onError: (error) => {
    toast.error(`清除失败: ${extractErrorMessage(error, '未知错误')}`);
  },
});
const { isPending: isRegenerating, run: runRegenerateRootCA } = useAsyncAction({
  onError: (error) => {
    toast.error(`重新生成失败: ${extractErrorMessage(error, '未知错误')}`);
  },
});
const { isPending: isDownloading, run: runDownloadFile } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '下载失败'));
  },
});
const { run: runRefreshCAStatus } = useAsyncAction({
  onError: () => {
    hasRootCA.value = false;
    caInfo.value = null;
    hosts.value = [];
  },
});

onMounted(() => {
  refreshCAStatus();
});

const isIP = (v: string) => {
  const s = v.trim();
  const noPort: string = s.includes(':') ? (s.split(':')[0] || s) : s;
  return /^(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)(?:\.|$)){4}$/.test(noPort);
};

async function addHost() {
  const entries = pendingHosts.value;
  if (!entries.length) return;
  await runBusyAction(
    async () => {
      for (const entry of entries) {
        hosts.value = await ConfigAPI.addCAHost(entry);
      }
    },
    {
      onSuccess: () => {
        newHost.value = '';
        toast.success(entries.length > 1 ? `已添加 ${entries.length} 个主机` : '已添加主机');
      },
      onError: (error) => {
        toast.error(`添加失败: ${extractErrorMessage(error, '未知错误')}`);
      },
    },
  );
}

async function confirmRemoveHost(value: string) {
  removingHost.value = value;
  await runRemoveHostAction(
    () => ConfigAPI.removeCAHost(value),
    {
      onSuccess: (nextHosts) => {
        hosts.value = nextHosts;
        toast.success('已移除主机');
      },
      onError: (error) => {
        toast.error(`移除失败: ${extractErrorMessage(error, '未知错误')}`);
      },
      onFinally: () => {
        removingHost.value = null;
      },
    },
  );
}

async function generateRootCA() {
  await runBusyAction(
    () => ConfigAPI.initCA(),
    {
      onSuccess: async () => {
        await refreshCAStatus();
        toast.success('根证书已生成并保存在后端');
      },
      onError: (error) => {
        toast.error(`生成失败: ${extractErrorMessage(error, '未知错误')}`);
      },
    },
  );
}

function openFirstConfirm() {
  showFirstConfirm.value = true;
}

function confirmFirst() {
  showFirstConfirm.value = false;
  showSecondConfirm.value = true;
}

async function confirmFinalClear() {
  await runClearRootCA(
    () => ConfigAPI.clearCA(),
    {
      onSuccess: () => {
      caInfo.value = null;
      hasRootCA.value = false;
      toast.success('已清除根证书');
      showSecondConfirm.value = false;
      },
    },
  );
}

function openRegenFirstConfirm() {
  showRegenFirstConfirm.value = true;
}

function confirmRegenFirst() {
  showRegenFirstConfirm.value = false;
  showRegenSecondConfirm.value = true;
}

async function confirmFinalRegen() {
  await runRegenerateRootCA(
    () => ConfigAPI.initCA(),
    {
      onSuccess: async () => {
        await refreshCAStatus();
        toast.success('已重新生成根证书');
        showRegenSecondConfirm.value = false;
      },
    },
  );
}

async function refreshCAStatus() {
  await runRefreshCAStatus(
    async () => {
      const { initialized, info } = await ConfigAPI.getCAStatus();
      hasRootCA.value = initialized;
      caInfo.value = info || null;
      hosts.value = await ConfigAPI.getCAHosts();
    },
    {
      onFinally: () => {
        isInitializing.value = false;
      },
    },
  );
}

async function issueAndInstall() {
  if (!hasRootCA.value || !hosts.value.length) return;
  await runBusyAction(
    () => ConfigAPI.issueAndInstall(),
    {
      onSuccess: ({ success, message }) => {
        if (success) {
          toast.success('证书签发并已安装');
          return;
        }
        toast.error(`签发失败: ${message || '未知错误'}`);
      },
      onError: (error) => {
        toast.error(`签发失败: ${extractErrorMessage(error, '未知错误')}`);
      },
    },
  );
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

async function downloadCA() {
  await runDownloadFile(async () => {
    const blob = await ConfigAPI.downloadCACert();
    downloadBlob(blob, 'KCI-LNK-Root-CA.pem');
  });
}

async function downloadServer() {
  await runDownloadFile(async () => {
    const blob = await ConfigAPI.downloadServerCert();
    downloadBlob(blob, 'server-cert.zip');
  });
}
</script>
