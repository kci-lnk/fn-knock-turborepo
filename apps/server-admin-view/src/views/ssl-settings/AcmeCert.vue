<template>
  <div class="grid gap-4 lg:grid-cols-[420px_1fr] items-start">
    <Card class="overflow-hidden">
      <CardHeader>
        <div class="flex items-start justify-between gap-4">
          <div class="grid gap-1">
            <CardTitle class="flex items-center gap-2">
              ACME 证书申请
            </CardTitle>
            <CardDescription>使用 DNS API 自动完成域名验证并签发证书。</CardDescription>
          </div>
          <RefreshButton
            :loading="isLoadingProviders || isAcmeFetching"
            :disabled="isLoadingProviders || isAcmeFetching || isAcmeInitializing"
            @click="refresh"
          />
        </div>
      </CardHeader>
      <CardContent v-if="isAcmeInitializing && showInitializingSkeleton" class="grid gap-6">
        <div class="grid gap-2">
          <Skeleton class="h-4 w-12" />
          <Skeleton class="h-9 w-full" />
        </div>
        <div class="grid gap-2">
          <Skeleton class="h-4 w-20" />
          <Skeleton class="h-9 w-full" />
        </div>
        <div class="grid gap-3 rounded-lg border bg-muted/20 p-4">
          <div class="flex items-center justify-between gap-3">
            <Skeleton class="h-4 w-24" />
            <Skeleton class="h-8 w-8 rounded-md" />
          </div>
          <div class="grid gap-3">
            <Skeleton class="h-9 w-full" />
            <Skeleton class="h-9 w-full" />
          </div>
        </div>
        <div class="flex items-center justify-end gap-2">
          <Skeleton class="h-9 w-16" />
          <Skeleton class="h-9 w-20" />
          <Skeleton class="h-9 w-20" />
        </div>
      </CardContent>
      <CardContent v-else-if="!isAcmeInitializing" class="grid gap-6">

        <div class="grid gap-2">
          <label class="text-sm text-muted-foreground">域名</label>
          <TagsInput class="min-h-[65px]" v-model="domains" :disabled="!isAcmeInstalled || isSubmitting || isSaving">
            <TagsInputItem v-for="item in domains" :key="item" :value="item">
              <TagsInputItemText />
              <TagsInputItemDelete />
            </TagsInputItem>
            <TagsInputInput :disabled="!isAcmeInstalled || isSubmitting || isSaving"
              placeholder="输入域名后按回车添加多个 (例如: example.com)" />
          </TagsInput>
        </div>

        <div class="grid gap-2">
          <div class="flex items-center justify-between gap-3">
            <label class="text-sm text-muted-foreground">DNS 服务商</label>
            <span v-if="activeDnsType" class="text-xs font-mono text-muted-foreground">{{ activeDnsType }}</span>
          </div>
          <Select v-model="dnsType" :disabled="!isAcmeInstalled || isSubmitting || isSaving">
            <SelectTrigger class="w-full">
              <SelectValue placeholder="选择 DNS 服务商" />
            </SelectTrigger>
            <SelectContent class="max-h-[320px]">
              <SelectGroup v-for="g in groupedProviders" :key="g.group">
                <SelectLabel>{{ g.group }}</SelectLabel>
                <SelectItem v-for="p in g.items" :key="p.dnsType" :value="p.dnsType">
                  <div class="flex w-full items-center justify-between gap-3">
                    <span class="truncate">{{ p.label }}</span>
                    <span class="shrink-0 font-mono text-xs text-muted-foreground">{{ p.dnsType }}</span>
                  </div>
                </SelectItem>
              </SelectGroup>
            </SelectContent>
          </Select>
        </div>

        <div v-if="activeProvider?.envKeys?.length" data-acme="credentials"
          class="grid gap-3 rounded-lg border bg-muted/20 p-4">
          <div class="flex items-center justify-between gap-3">
            <div class="grid gap-0.5">
              <div class="text-sm font-medium">DNS API 凭据</div>
            </div>
            <Button type="button" variant="ghost" size="icon-sm" class="text-muted-foreground hover:text-foreground"
              :title="isCredentialsVisible ? '隐藏' : '显示'" :aria-label="isCredentialsVisible ? '隐藏凭据' : '显示凭据'"
              @click="isCredentialsVisible = !isCredentialsVisible">
              <component :is="isCredentialsVisible ? EyeOff : Eye" class="h-4 w-4" />
            </Button>
          </div>

          <div v-if="activeProvider?.envKeys?.length" class="grid gap-3">
            <div v-for="(k, index) in activeProvider.envKeys" :key="k" class="grid gap-2">
              <div class="flex items-center justify-between gap-2">
                <span class="text-sm font-mono text-muted-foreground">{{ k }}</span>
              </div>
              <Input v-model.trim="credentials[k]" :type="isCredentialsVisible ? 'text' : 'password'" class="font-mono"
                :name="`acme-credential-${index}`" autocomplete="new-password"
                :readonly="!isCredentialEditReady(k)" :disabled="!isAcmeInstalled || isSubmitting || isSaving"
                @focus="enableCredentialEditing(k)" @pointerdown="enableCredentialEditing(k)" />
            </div>
          </div>
        </div>

        <div class="flex items-center justify-end gap-2">
          <Button variant="outline" :disabled="isSubmitting" @click="resetForm">清空</Button>
          <Button variant="secondary" :disabled="!isAcmeInstalled || !canSubmit || isSubmitting || isSaving"
            @click="save">
            <span v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            保存
          </Button>
          <Button :disabled="!isAcmeInstalled || !canSubmit || isSubmitting || isSaving" @click="submit">
            <span v-if="isSubmitting"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"></span>
            申请证书
          </Button>
        </div>
      </CardContent>
      <CardContent v-else class="min-h-[360px]" aria-hidden="true" ></CardContent>
    </Card>

    <div class="grid gap-4">
      <Card v-if="!isAcmeInitializing && !isAcmeInstalled">
        <CardHeader>
          <CardTitle>ACME.sh 未就绪</CardTitle>
          <CardDescription>申请证书前需要先安装并初始化 acme.sh。</CardDescription>
        </CardHeader>
        <CardContent class="grid gap-2 text-sm text-muted-foreground">
          <div v-if="acmeState?.status === 'installing'">安装中，请稍候…</div>
          <div v-else-if="acmeState?.status === 'error'">错误：{{ acmeState?.message || '未知错误' }}</div>
          <div v-else-if="acmeState?.status === 'uninstalled'">当前未检测到 acme.sh，请先完成安装。</div>
          <div v-else>无法获取 acme.sh 状态，请检查服务是否正常。</div>
        </CardContent>
        <CardFooter class="flex justify-end gap-2 border-t pt-6">
          <RefreshButton label="刷新状态" :loading="isAcmeFetching" :disabled="isAcmeFetching" @click="fetchAcmeStatus" />
          <Button @click="gotoAcmeSsl">{{ acmeDialogActionLabel }}</Button>
        </CardFooter>
      </Card>

      <Card v-if="job">
        <CardHeader>
          <div class="flex items-start justify-between gap-4">
            <div class="grid gap-1">
              <CardTitle class="flex items-center gap-2">
                签发任务
                <Badge :variant="jobBadgeVariant">{{ jobStatusLabel }}</Badge>
              </CardTitle>
              <CardDescription class="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span class="font-mono text-xs">{{ job.domains?.join(', ') }}</span>
                <span class="text-xs text-muted-foreground">·</span>
                <span class="font-mono text-xs text-muted-foreground">{{ job.provider || '-' }}</span>
              </CardDescription>
            </div>
            <RefreshButton label="刷新日志" :loading="isRefreshingLogs" :disabled="!job || isRefreshingLogs" @click="refreshLogs" />
          </div>
        </CardHeader>
        <CardContent class="grid gap-4">
          <Alert v-if="analysis" :variant="analysisVariant">
            <component :is="analysisIcon" class="h-4 w-4" />
            <AlertTitle>{{ analysisTitle }}</AlertTitle>
            <AlertDescription>
              <div class="grid gap-2">
                <p>{{ analysis.message }}</p>
                <div class="flex flex-wrap items-center gap-2">
                  <Button
                    v-if="analysis.reason === 'dns_credentials_invalid' || analysis.reason === 'dns_credentials_invalid_email'"
                    type="button" variant="outline" size="sm" @click="focusCredentials">
                    检查 DNS 凭据
                  </Button>
                  <Button v-if="analysis.evidence?.length" type="button" variant="ghost" size="sm" class="px-2"
                    @click="isAnalysisOpen = !isAnalysisOpen">
                    {{ isAnalysisOpen ? '收起' : '查看' }}
                  </Button>
                </div>

                <Collapsible v-if="analysis.evidence?.length" v-model:open="isAnalysisOpen">
                  <CollapsibleContent>
                    <div
                      class="rounded-md border bg-muted/20 p-2 font-mono text-xs text-muted-foreground w-full min-w-0">
                      <div v-for="(line, idx) in analysis.evidence" :key="idx" class="whitespace-pre-wrap break-all">{{
                        line }}</div>
                    </div>
                  </CollapsibleContent>
                </Collapsible>
              </div>
            </AlertDescription>
          </Alert>
          <div class="grid gap-2">
            <div class="flex items-center justify-between">
              <span class="text-xs text-muted-foreground">进度</span>
              <span class="text-xs font-mono text-muted-foreground">{{ jobProgress }}%</span>
            </div>
            <Progress :model-value="jobProgress" />
          </div>
          <LogViewer :logs="logs" />
        </CardContent>
        <CardFooter v-if="job.status === 'succeeded'" class="flex flex-wrap gap-2 justify-end border-t pt-6">
          <Button variant="secondary" @click="download" :disabled="isDownloading">下载证书</Button>
          <Button @click="deploy" :disabled="isDeploying">一键部署</Button>
          <Button variant="outline" @click="reapply">重新申请</Button>
        </CardFooter>
      </Card>

      <Card v-if="certInfo">
        <CardHeader>
          <CardTitle>证书信息 <Badge variant="default" class="bg-green-600 hover:bg-green-600">
              <svg xmlns="http://www.w3.org/2000/svg" class="mr-1 h-3 w-3" viewBox="0 0 24 24" fill="none"
                stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" />
                <path d="m9 12 2 2 4-4" />
              </svg>
              已申请
            </Badge>
          </CardTitle>
          <CardDescription>

            已签发并保存到服务器，可下载或部署。
          </CardDescription>
        </CardHeader>
        <CardContent class="grid gap-2 text-sm">
          <div class="grid grid-cols-[120px_1fr] gap-y-1">
            <span class="text-muted-foreground">主域名</span>
            <span class="font-mono text-xs break-all">{{ certInfo.primaryDomain }}</span>
            <span class="text-muted-foreground">发行者</span>
            <span class="font-mono text-xs break-all">{{ certInfo.info?.issuer }}</span>
            <span class="text-muted-foreground">有效期</span>
            <span class="font-mono text-xs">{{ formatDate(certInfo.info?.validFrom) }} ~ {{
              formatDate(certInfo.info?.validTo) }}</span>
            <span class="text-muted-foreground">包含名称</span>
            <span class="font-mono text-xs break-words">{{ (certInfo.info?.dnsNames || []).join(', ') }}</span>
            <span class="text-muted-foreground">序列号</span>
            <span class="font-mono text-xs break-all text-muted-foreground">{{ certInfo.info?.serialNumber }}</span>
          </div>
        </CardContent>
        <CardFooter class="flex flex-wrap justify-end gap-2 border-t pt-6">
          <Button variant="secondary" @click="download" :disabled="isDeleting">下载证书</Button>
          <Button @click="deploy" :disabled="isDeleting">一键部署</Button>
          <ConfirmDangerPopover
            title="确认删除 ACME 证书"
            description="删除后将移除服务器保存的证书与私钥文件，且可能会禁用当前正在使用的同一份证书。"
            :loading="isDeleting"
            :disabled="isDeleting"
            :on-confirm="confirmDelete"
            content-class="w-96 text-left"
          >
            <template #trigger>
              <Button variant="destructive" :disabled="isDeleting">删除</Button>
            </template>
          </ConfirmDangerPopover>
        </CardFooter>
      </Card>

      <Card v-if="isAcmeInstalled && !job && !certInfo">
        <CardHeader>
          <CardTitle>提示</CardTitle>
          <CardDescription>选择 DNS 服务商并填写凭据后提交任务。</CardDescription>
        </CardHeader>
        <CardContent class="grid gap-2 text-sm text-muted-foreground">
          <div>提交后会自动执行 DNS 验证并签发证书，可在右侧查看进度与日志。</div>
        </CardContent>
      </Card>
    </div>
  </div>

  <Dialog v-model:open="showAcmeInstallDialog">
    <DialogContent>
      <DialogHeader>
        <DialogTitle>{{ acmeDialogTitle }}</DialogTitle>
      </DialogHeader>
      <p class="text-sm text-muted-foreground">{{ acmeDialogDescription }}</p>
      <DialogFooter>
        <Button @click="gotoAcmeSsl">{{ acmeDialogActionLabel }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch, nextTick } from 'vue';
import { useRouter } from 'vue-router';
import { Card, CardHeader, CardTitle, CardDescription, CardContent, CardFooter } from '@/components/ui/card';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import RefreshButton from '@/components/RefreshButton.vue';
import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Collapsible, CollapsibleContent } from '@/components/ui/collapsible';
import { Input } from '@/components/ui/input';
import { Select, SelectContent, SelectGroup, SelectItem, SelectLabel, SelectTrigger, SelectValue } from '@/components/ui/select';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { TagsInput, TagsInputInput, TagsInputItem, TagsInputItemDelete, TagsInputItemText } from '@/components/ui/tags-input';
import { Dialog, DialogContent, DialogFooter, DialogHeader, DialogTitle } from '@/components/ui/dialog';
import { toast } from '@admin-shared/utils/toast';
import { Eye, EyeOff, TriangleAlert, Info } from 'lucide-vue-next';
import { AcmeAPI } from '../../lib/api';
import LogViewer from '@admin-shared/components/LogViewer.vue';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { downloadBlob } from '@admin-shared/utils/downloadBlob';

type AcmeState = { status: 'uninstalled' | 'installing' | 'installed' | 'error'; progress: number; message: string };
type AcmeLogAnalysis = {
  reason: "dns_credentials_invalid" | "dns_credentials_invalid_email" | "dns_api_rate_limited" | "acme_frequency_limited" | "unknown";
  provider?: string;
  message: string;
  evidence?: string[];
};

const router = useRouter();

const acmeState = ref<AcmeState | null>(null);
const isAcmeInitializing = ref(true);
const showInitializingSkeleton = useDelayedLoading(isAcmeInitializing);
const showAcmeInstallDialog = ref(false);

const domains = ref<string[]>([]);
const credentials = ref<Record<string, string>>({});
const isCredentialsVisible = ref(false);
const credentialEditReady = ref<Record<string, boolean>>({});
const job = ref<any>(null);
const logs = ref<string[]>([]);
const analysis = ref<AcmeLogAnalysis | null>(null);
const isAnalysisOpen = ref(false);
let timer: any = null;
let savePromise: Promise<void> | null = null;
const certInfo = ref<{ primaryDomain: string; info: any } | null>(null);
const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
  rethrow: true,
});
const { isPending: isSubmitting, run: runSubmitRequest } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '提交失败'));
  },
});
const { isPending: isDeleting, run: runDeleteCert } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '删除失败'));
  },
});
const { isPending: isRefreshingLogs, run: runRefreshLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '刷新日志失败'));
  },
});
const { isPending: isDownloading, run: runDownloadCert } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '下载失败'));
  },
});
const { isPending: isDeploying, run: runDeployCert } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '部署失败'));
  },
});
const { run: runLoadSavedConfig } = useAsyncAction();
const { isPending: isAcmeFetching, run: runFetchAcmeStatus } = useAsyncAction();

type DnsProvider = { dnsType: string; label: string; group: string; envKeys: string[] };

const dnsProviders = ref<DnsProvider[]>([]);
const { isPending: isLoadingProviders, run: runLoadProviders } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '加载 DNS 服务商失败'));
    dnsProviders.value = [];
  },
});
const dnsType = ref<string>('');
const customDnsType = ref('');

const activeProvider = computed(() => {
  if (!dnsType.value || dnsType.value === '__custom__') return null;
  return dnsProviders.value.find(p => p.dnsType === dnsType.value) || null;
});

const activeDnsType = computed(() => {
  if (dnsType.value === '__custom__') return customDnsType.value.trim();
  return dnsType.value.trim();
});

const getCredentialStateKey = (key: string) => `${activeDnsType.value}:${key}`;

const enableCredentialEditing = (key: string) => {
  credentialEditReady.value[getCredentialStateKey(key)] = true;
};

const isCredentialEditReady = (key: string) => credentialEditReady.value[getCredentialStateKey(key)] === true;

const groupedProviders = computed(() => {
  const groupOrder = ['常用', '国内', '国际', '自建/高级'];
  const bucket = new Map<string, DnsProvider[]>();
  for (const p of dnsProviders.value) {
    const g = p.group || '其他';
    if (!bucket.has(g)) bucket.set(g, []);
    bucket.get(g)!.push(p);
  }

  const groups = Array.from(bucket.entries()).map(([group, items]) => ({
    group,
    items: items.slice().sort((a, b) => a.label.localeCompare(b.label, 'zh-Hans-CN')),
  }));

  groups.sort((a, b) => {
    const ai = groupOrder.indexOf(a.group);
    const bi = groupOrder.indexOf(b.group);
    if (ai === -1 && bi === -1) return a.group.localeCompare(b.group, 'zh-Hans-CN');
    if (ai === -1) return 1;
    if (bi === -1) return -1;
    return ai - bi;
  });

  return groups;
});

const isAcmeInstalled = computed(() => acmeState.value?.status === 'installed');

const analysisVariant = computed(() => {
  if (!analysis.value) return 'default';
  if (analysis.value.reason === 'unknown') return 'default';
  return 'destructive';
});

const analysisIcon = computed(() => {
  if (!analysis.value) return Info;
  if (analysisVariant.value === 'destructive') return TriangleAlert;
  return Info;
});

const analysisTitle = computed(() => {
  const a = analysis.value;
  if (!a) return '';
  if (a.reason === 'dns_credentials_invalid' || a.reason === 'dns_credentials_invalid_email') return 'DNS 凭据可能有问题';
  if (a.reason === 'dns_api_rate_limited') return 'DNS API 触发限流';
  if (a.reason === 'acme_frequency_limited') return '申请频率受限';
  return '检测到异常信息';
});

const acmeDialogTitle = computed(() => {
  const s = acmeState.value?.status;
  if (s === 'installing') return 'ACME.sh 安装中';
  if (s === 'error') return 'ACME.sh 状态异常';
  if (s === 'installed') return 'ACME.sh 已安装';
  return 'ACME.sh 未安装';
});

const acmeDialogDescription = computed(() => {
  const s = acmeState.value?.status;
  if (s === 'installing') return '正在安装 acme.sh，请稍候或前往系统设置查看进度。';
  if (s === 'error') return acmeState.value?.message || 'acme.sh 状态异常，请前往系统设置重新安装。';
  if (!s) return '无法获取 acme.sh 状态，请检查服务是否正常。';
  return '请先在 系统设置 → ACME SSL 页面完成安装，安装完成后返回此页继续申请证书。';
});

const acmeDialogActionLabel = computed(() => {
  const s = acmeState.value?.status;
  if (s === 'installing') return '查看进度';
  if (s === 'error') return '前往处理';
  return '前往安装';
});

function gotoAcmeSsl() {
  showAcmeInstallDialog.value = false;
  router.push({ path: '/system', query: { tab: 'acme-ssl' } });
}

async function fetchAcmeStatus(opts?: { silent?: boolean }) {
  await runFetchAcmeStatus(
    async () => {
      const st = await AcmeAPI.status();
      acmeState.value = st;
      if (!job.value) certInfo.value = st.acmeCert ?? null;
    },
    {
      onError: (error) => {
        acmeState.value = null;
        if (!opts?.silent) {
          toast.error(extractErrorMessage(error, '获取 ACME.sh 状态失败'));
        }
      },
      onFinally: () => {
        isAcmeInitializing.value = false;
      },
    },
  );

  if (acmeState.value?.status === 'uninstalled') {
    showAcmeInstallDialog.value = true;
  }
}

async function refresh() {
  await fetchAcmeStatus();
  if (isAcmeInstalled.value) await loadProviders();
}

async function loadProviders() {
  if (!isAcmeInstalled.value) {
    showAcmeInstallDialog.value = true;
    return;
  }
  await runLoadProviders(
    () => AcmeAPI.dnsProviders(),
    {
      onSuccess: (providers) => {
        dnsProviders.value = providers;
      },
    },
  );
}

const canSubmit = computed(() => {
  if (domains.value.length === 0) return false;
  const v = activeDnsType.value;
  if (!v) return false;
  if (!/^dns_[a-z0-9_]+$/i.test(v)) return false;
  return true;
});

const jobProgress = computed(() => {
  const v = Number(job.value?.progress ?? 0);
  if (!Number.isFinite(v)) return 0;
  return Math.max(0, Math.min(100, Math.round(v)));
});

const jobStatusLabel = computed(() => {
  const s = String(job.value?.status || '');
  if (s === 'queued') return '排队中';
  if (s === 'running') return '执行中';
  if (s === 'succeeded') return '已完成';
  if (s === 'failed') return '失败';
  return s || '未知';
});

const jobBadgeVariant = computed(() => {
  const s = String(job.value?.status || '');
  if (s === 'succeeded') return 'secondary';
  if (s === 'failed') return 'destructive';
  if (s === 'running') return 'default';
  return 'outline';
});

const buildCredentialsPayload = () => {
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(credentials.value || {})) {
    const kk = k.trim();
    const vv = (v ?? '').toString().trim();
    if (kk && vv) out[kk] = vv;
  }
  return out;
};

async function save(opts?: { silent?: boolean }) {
  if (!canSubmit.value) return;
  if (savePromise) return await savePromise;

  const action = runSaveConfig(async () => {
    await AcmeAPI.saveConfig({
      domains: domains.value,
      dnsType: activeDnsType.value,
      credentials: buildCredentialsPayload()
    });
  }, {
    onSuccess: () => {
      if (!opts?.silent) toast.success('已保存');
    },
    onError: (error) => {
      if (!opts?.silent) toast.error(extractErrorMessage(error, '保存失败'));
    },
  });
  if (!action) return;
  savePromise = action;
  try {
    await savePromise;
  } finally {
    savePromise = null;
  }
}

async function loadSavedConfig() {
  await runLoadSavedConfig(async () => {
    const cfg = await AcmeAPI.getConfig();
    if (!cfg) return;
    domains.value = Array.isArray(cfg.domains) ? cfg.domains : [];
    dnsType.value = String(cfg.dnsType || '');
    credentials.value = { ...(cfg.credentials || {}) };
  });
}

async function submit() {
  if (!canSubmit.value) return;
  await runSubmitRequest(async () => {
    logs.value = [];
    job.value = null;
    analysis.value = null;
    isAnalysisOpen.value = false;
    certInfo.value = null;
    if (timer) clearInterval(timer);
    await save({ silent: true });
    const { jobId } = await AcmeAPI.request({
      domains: domains.value,
      dnsType: activeDnsType.value,
      credentials: buildCredentialsPayload()
    });
    toast.success('任务已提交');
    await pollOnce(jobId);
    startPolling(jobId);
  });
}

async function pollOnce(id: string) {
  const data = await AcmeAPI.poll(id, { limit: 500, order: 'desc' });
  job.value = data.job;
  logs.value = data.logs;
  analysis.value = data.analysis ?? null;
  if (!analysis.value) isAnalysisOpen.value = false;
}

async function refreshLogs() {
  if (!job.value) return;
  await runRefreshLogs(() => pollOnce(job.value.id));
}

function startPolling(id: string) {
  clearInterval(timer);
  timer = setInterval(async () => {
    try {
      await pollOnce(id);
      if (job.value?.status === 'succeeded' || job.value?.status === 'failed') {
        clearInterval(timer);
        if (job.value?.status === 'succeeded') {
          const primaryDomain = job.value.domains[0];
          certInfo.value = {
            primaryDomain,
            info: await AcmeAPI.certInfo(primaryDomain).then(res => res.info)
          };
        }
      }
    } catch { }
  }, 2000);
}

function resetForm() {
  domains.value = [];
  dnsType.value = '';
  customDnsType.value = '';
  credentials.value = {};
  credentialEditReady.value = {};
  isCredentialsVisible.value = false;
  logs.value = [];
  job.value = null;
  analysis.value = null;
  isAnalysisOpen.value = false;
  certInfo.value = null;
  clearInterval(timer);
}

async function focusCredentials() {
  isCredentialsVisible.value = true;
  await nextTick();
  const el = document.querySelector('[data-acme="credentials"]') as HTMLElement | null;
  el?.scrollIntoView({ behavior: 'smooth', block: 'start' });
}

async function download() {
  const primaryDomain = resolvePrimaryDomain();
  if (!primaryDomain) return;
  await runDownloadCert(async () => {
    const blob = await AcmeAPI.download(primaryDomain);
    downloadBlob(blob, `${primaryDomain}.zip`);
  });
}

async function deploy() {
  const primaryDomain = resolvePrimaryDomain();
  if (!primaryDomain) return;
  await runDeployCert(async () => {
    await AcmeAPI.deploy(primaryDomain);
    toast.success('已提交部署');
  });
}

function resolvePrimaryDomain() {
  const d1 = job.value?.domains?.[0];
  if (typeof d1 === 'string' && d1) return d1;
  const d2 = certInfo.value?.primaryDomain;
  if (typeof d2 === 'string' && d2) return d2;
  return null;
}

async function confirmDelete() {
  const primaryDomain = resolvePrimaryDomain();
  if (!primaryDomain) return;
  await runDeleteCert(async () => {
    await AcmeAPI.deleteCert(primaryDomain);
    certInfo.value = null;
    if (job.value?.domains?.[0] === primaryDomain) job.value = null;
    toast.success('已删除证书');
    await fetchAcmeStatus({ silent: true });
  });
}

async function reapply() {
  await submit();
}

function formatDate(v: string) {
  if (!v) return '';
  const date = new Date(v);
  if (Number.isNaN(date.getTime())) return v;
  return date.toLocaleString();
}

onUnmounted(() => { if (timer) clearInterval(timer); });

watch(dnsType, () => {
  credentialEditReady.value = {};
  const next = dnsProviders.value.find(p => p.dnsType === dnsType.value)?.envKeys || [];
  if (!next.length) {
    credentials.value = {};
    isCredentialsVisible.value = false;
    return;
  }
  const prev = { ...(credentials.value || {}) };
  const out: Record<string, string> = {};
  for (const k of next) {
    const v = prev[k];
    out[k] = typeof v === 'string' ? v : '';
  }
  credentials.value = out;
  isCredentialsVisible.value = false;
});

onMounted(async () => {
  await fetchAcmeStatus({ silent: true });
  if (isAcmeInstalled.value) {
    await loadProviders();
    await loadSavedConfig();
  }
});
</script>
