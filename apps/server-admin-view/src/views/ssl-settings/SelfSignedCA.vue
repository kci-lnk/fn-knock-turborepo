<template>
  <div class="grid gap-4">
    <Card v-if="isInitializing && showInitializingSkeleton">
      <CardHeader>
        <CardTitle>{{ t('admin.selfSignedCA.rootTitle') }}</CardTitle>
        <CardDescription>{{ t('admin.selfSignedCA.rootDescription') }}</CardDescription>
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
        <CardTitle>{{ t('admin.selfSignedCA.rootTitle') }}</CardTitle>
        <CardDescription>{{ t('admin.selfSignedCA.rootDescription') }}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-4">
        <Alert
          v-if="!hasRootCA"
          variant="destructive"
          class="dynamic-white-glass-surface"
        >
          <AlertTitle>{{ t('admin.selfSignedCA.notInitializedTitle') }}</AlertTitle>
          <AlertDescription>{{ t('admin.selfSignedCA.notInitializedDescription') }}</AlertDescription>
        </Alert>
        <div v-else class="rounded-lg border bg-muted/30 p-4 grid gap-3 text-sm">
          <Badge
            variant="default"
            class="dynamic-white-glass-chip dynamic-white-glass-chip-success bg-green-600 hover:bg-green-600"
          >
            <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-3 w-3" viewBox="0 0 24 24" fill="none"
              stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
              <path d="m9 12 2 2 4-4" />
            </svg>
            {{ t('admin.selfSignedCA.rootCertificate') }}
          </Badge>
          <div class="grid grid-cols-[110px_1fr] gap-y-2">
            <span class="text-muted-foreground font-medium">{{ t('admin.selfSignedCA.subject') }}</span>
            <span class="font-mono text-xs break-all">{{ caInfo?.subject }}</span>
            <span class="text-muted-foreground font-medium">{{ t('admin.selfSignedCA.issuer') }}</span>
            <span class="font-mono text-xs break-all">{{ caInfo?.issuer }}</span>
            <span class="text-muted-foreground font-medium">{{ t('admin.selfSignedCA.validity') }}</span>
            <span class="text-xs">
              <span>{{ caInfo ? formatDate(caInfo.validFrom) : '' }}</span>
              <span class="mx-1 text-muted-foreground">{{ t('admin.selfSignedCA.to') }}</span>
              <span>{{ caInfo ? formatDate(caInfo.validTo) : '' }}</span>
            </span>
            <span class="text-muted-foreground font-medium">{{ t('admin.selfSignedCA.serialNumber') }}</span>
            <span class="font-mono text-xs break-all text-muted-foreground">{{ caInfo?.serialNumber }}</span>
          </div>
        </div>
      </CardContent>
      <CardFooter class="flex gap-2">
        <Button v-if="!hasRootCA" @click="generateRootCA" :disabled="isBusy">
          <span v-if="isBusy"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          {{ t('admin.selfSignedCA.initializeRoot') }}
        </Button>
        <template v-else>

          <div class="inline-flex items-stretch">
            <ButtonGroup>
              <Button variant="outline" @click="downloadCA" :disabled="isBusy || isDownloading">{{ t('admin.selfSignedCA.downloadRoot') }}</Button>
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button variant="outline" size="icon" :aria-label="t('admin.selfSignedCA.moreActions')" :disabled="isBusy || isDownloading">
                    <MoreHorizontal class="h-4 w-4" />
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" class="w-52">
                  <DropdownMenuGroup>
                    <DropdownMenuItem @click="openRegenFirstConfirm">
                      <RefreshCw class="mr-2 h-4 w-4" />
                      {{ t('admin.selfSignedCA.regenerate') }}
                    </DropdownMenuItem>
                  </DropdownMenuGroup>
                  <DropdownMenuSeparator />
                  <DropdownMenuGroup>
                    <DropdownMenuItem variant="destructive" @click="openFirstConfirm">
                      <Trash2 class="mr-2 h-4 w-4" />
                      {{ t('admin.selfSignedCA.clearRoot') }}
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
        <CardTitle>{{ t('admin.selfSignedCA.hostListTitle') }}</CardTitle>
        <CardDescription>{{ t('admin.selfSignedCA.hostListDescription') }}</CardDescription>
      </CardHeader>
      <CardContent class="grid gap-3">
        <div class="flex gap-2">
          <Input v-model="newHost" :placeholder="t('admin.selfSignedCA.hostPlaceholder')" @keydown.enter.prevent="addHost" />
          <Button @click="addHost" :disabled="!pendingHosts.length">{{ t('admin.selfSignedCA.add') }}</Button>
        </div>
        <div class="rounded-md border overflow-hidden">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead class="w-[60px]">#</TableHead>
                <TableHead>{{ t('admin.selfSignedCA.hostOrIp') }}</TableHead>
                <TableHead class="w-[120px]">{{ t('admin.selfSignedCA.type') }}</TableHead>
                <TableHead class="w-[100px]">{{ t('admin.selfSignedCA.actions') }}</TableHead>
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
                    :title="t('admin.selfSignedCA.confirmRemoveHostTitle')"
                    :description="t('admin.selfSignedCA.confirmRemoveHostDescription')"
                    :confirm-text="t('admin.selfSignedCA.confirmRemove')"
                    :loading="isRemoving && removingHost === h"
                    :disabled="isRemoving && removingHost === h"
                    :on-confirm="() => confirmRemoveHost(h)"
                    content-class="w-72 text-left"
                  >
                    <template #trigger>
                      <Button size="sm" variant="ghost" :disabled="isRemoving && removingHost === h">{{ t('admin.selfSignedCA.remove') }}</Button>
                    </template>
                  </ConfirmDangerPopover>
                </TableCell>
              </TableRow>
              <TableRow v-if="!hosts.length">
                <TableCell colspan="4" class="text-center text-muted-foreground">{{ t('admin.selfSignedCA.noEntries') }}</TableCell>
              </TableRow>
            </TableBody>
          </Table>
        </div>
      </CardContent>
      <CardFooter class="flex justify-end">
        <Button @click="issueAndInstall" :disabled="!hasRootCA || !hosts.length || isBusy">
          <span v-if="isBusy"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
          {{ t('admin.selfSignedCA.deploy') }}
        </Button>
        <Button variant="outline" class="ml-2" @click="downloadServer" :disabled="isBusy || isDownloading">{{ t('admin.selfSignedCA.downloadCertificate') }}</Button>
      </CardFooter>
    </Card>
    <Card v-else-if="showInitializingSkeleton">
      <CardHeader>
        <CardTitle>{{ t('admin.selfSignedCA.hostListTitle') }}</CardTitle>
        <CardDescription>{{ t('admin.selfSignedCA.hostListDescription') }}</CardDescription>
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
                <TableHead>{{ t('admin.selfSignedCA.hostOrIp') }}</TableHead>
                <TableHead class="w-[120px]">{{ t('admin.selfSignedCA.type') }}</TableHead>
                <TableHead class="w-[100px]">{{ t('admin.selfSignedCA.actions') }}</TableHead>
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
          <DialogTitle>{{ t('admin.selfSignedCA.confirmClearTitle') }}</DialogTitle>
          <DialogDescription>{{ t('admin.selfSignedCA.confirmClearDescription') }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showFirstConfirm = false">{{ t('common.cancel') }}</Button>
          <Button variant="destructive" @click="confirmFirst" :disabled="isClearing">
            <span v-if="isClearing"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            {{ t('admin.selfSignedCA.nextStep') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showSecondConfirm" @update:open="showSecondConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('admin.selfSignedCA.secondConfirmTitle') }}</DialogTitle>
          <DialogDescription>{{ t('admin.selfSignedCA.secondClearDescription') }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showSecondConfirm = false">{{ t('common.cancel') }}</Button>
          <Button variant="destructive" @click="confirmFinalClear" :disabled="isClearing">
            <span v-if="isClearing"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            {{ t('admin.selfSignedCA.confirmClear') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showRegenFirstConfirm" @update:open="showRegenFirstConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('admin.selfSignedCA.confirmRegenerateTitle') }}</DialogTitle>
          <DialogDescription>{{ t('admin.selfSignedCA.confirmRegenerateDescription') }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showRegenFirstConfirm = false">{{ t('common.cancel') }}</Button>
          <Button variant="destructive" @click="confirmRegenFirst" :disabled="isRegenerating">
            <span v-if="isRegenerating"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            {{ t('admin.selfSignedCA.nextStep') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
    <Dialog :open="showRegenSecondConfirm" @update:open="showRegenSecondConfirm = $event">
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{{ t('admin.selfSignedCA.secondConfirmTitle') }}</DialogTitle>
          <DialogDescription>{{ t('admin.selfSignedCA.secondRegenerateDescription') }}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" @click="showRegenSecondConfirm = false">{{ t('common.cancel') }}</Button>
          <Button variant="destructive" @click="confirmFinalRegen" :disabled="isRegenerating">
            <span v-if="isRegenerating"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            {{ t('admin.selfSignedCA.confirmRegenerate') }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue';
import { useI18n } from 'vue-i18n';
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

const { locale, t } = useI18n();
const newHost = ref('');
const hosts = ref<string[]>([]);
const parseHosts = (value: string) =>
  value
    .split(/[\uFF0C,]/g)
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
    toast.error(t('admin.selfSignedCA.clearFailed'), {
      description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
    });
  },
});
const { isPending: isRegenerating, run: runRegenerateRootCA } = useAsyncAction({
  onError: (error) => {
    toast.error(t('admin.selfSignedCA.regenerateFailed'), {
      description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
    });
  },
});
const { isPending: isDownloading, run: runDownloadFile } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.selfSignedCA.downloadFailed')));
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
        toast.success(
          entries.length > 1
            ? t('admin.selfSignedCA.hostsAdded', { count: entries.length })
            : t('admin.selfSignedCA.hostAdded'),
        );
      },
      onError: (error) => {
        toast.error(t('admin.selfSignedCA.addFailed'), {
          description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
        });
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
        toast.success(t('admin.selfSignedCA.hostRemoved'));
      },
      onError: (error) => {
        toast.error(t('admin.selfSignedCA.removeFailed'), {
          description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
        });
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
        toast.success(t('admin.selfSignedCA.rootGenerated'));
      },
      onError: (error) => {
        toast.error(t('admin.selfSignedCA.generateFailed'), {
          description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
        });
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
      toast.success(t('admin.selfSignedCA.rootCleared'));
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
        toast.success(t('admin.selfSignedCA.rootRegenerated'));
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
          toast.success(t('admin.selfSignedCA.certificateIssuedInstalled'));
          return;
        }
        toast.error(t('admin.selfSignedCA.issueFailed'), {
          description: message || t('admin.selfSignedCA.unknownError'),
        });
      },
      onError: (error) => {
        toast.error(t('admin.selfSignedCA.issueFailed'), {
          description: extractErrorMessage(error, t('admin.selfSignedCA.unknownError')),
        });
      },
    },
  );
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  if (Number.isNaN(date.getTime())) return dateStr;
  return date.toLocaleDateString(locale.value, {
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
