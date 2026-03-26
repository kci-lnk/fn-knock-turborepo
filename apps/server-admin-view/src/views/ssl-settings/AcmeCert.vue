<template>
  <div class="grid gap-4">
    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <div
          class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
        >
          <div class="grid gap-1">
            <CardTitle class="flex flex-wrap items-center gap-2">
              ACME 证书申请
              <Badge :variant="acmeStatusBadgeVariant">{{
                acmeStatusLabel
              }}</Badge>
              <Badge v-if="isTableLocked" variant="outline">
                {{ lockReasonLabel }}
              </Badge>
            </CardTitle>
            <CardDescription>
              管理多个 ACME 申请项、签发证书和证书库关联状态。
            </CardDescription>
          </div>
          <div class="flex flex-wrap items-center gap-2">
            <RefreshButton
              :loading="isOverviewLoading || isProvidersLoading"
              :disabled="isOverviewLoading || isProvidersLoading"
              @click="refresh"
            />
            <Button
              :disabled="
                isTableLocked || isDialogSubmitting || !dnsProviders.length
              "
              @click="openCreateDialog"
            >
              新申请
            </Button>
          </div>
        </div>
      </CardHeader>
    </Card>

    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <CardTitle>申请项列表</CardTitle>
        <CardDescription>
          列表中的每一行对应一条独立的 ACME 申请配置和它当前的证书状态。
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div class="relative">
          <div
            v-if="isTableLocked"
            class="absolute inset-0 z-10 flex items-center justify-center rounded-lg border bg-background/80 backdrop-blur-sm"
          >
            <div class="max-w-md text-center">
              <div class="text-sm font-medium">{{ lockMessageTitle }}</div>
              <div class="mt-1 text-xs text-muted-foreground">
                {{ lockMessageDescription }}
              </div>
            </div>
          </div>

          <div class="overflow-x-auto rounded-lg border">
            <Table class="table-fixed">
              <TableHeader>
                <TableRow>
                  <TableHead class="w-[100px] whitespace-normal">
                    DNS 服务商
                  </TableHead>
                  <TableHead class="w-[120px] whitespace-normal">域名</TableHead>
                  <TableHead class="w-[180px] whitespace-normal">
                    状态概览
                  </TableHead>
                  <TableHead class="w-[150px] whitespace-normal"
                    >有效期</TableHead
                  >
                  <TableHead class="w-[108px] whitespace-normal text-right">
                    操作
                  </TableHead>
                </TableRow>
              </TableHeader>

              <TableBody>
                <template v-if="isOverviewLoading && !applications.length">
                  <TableRow v-for="index in 4" :key="index">
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-16" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-36" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-24" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal">
                      <Skeleton class="h-4 w-24" />
                    </TableCell>
                    <TableCell class="align-top whitespace-normal text-right">
                      <div class="ml-auto inline-flex">
                        <Skeleton class="h-8 w-16 rounded-r-none" />
                        <Skeleton class="h-8 w-8 rounded-l-none border-l" />
                      </div>
                    </TableCell>
                  </TableRow>
                </template>

                <TableRow v-else-if="!applications.length">
                  <TableCell
                    colspan="5"
                    class="py-10 text-center text-muted-foreground"
                  >
                    还没有 ACME 申请项，点击右上角“新申请”开始创建。
                  </TableCell>
                </TableRow>

                <TableRow
                  v-for="application in applications"
                  :key="application.id"
                >
                  <TableCell class="align-top whitespace-normal break-words">
                    <div class="grid gap-1">
                      <div class="font-medium">
                        {{ application.providerLabel }}
                      </div>
                      <div class="font-mono text-xs text-muted-foreground">
                        {{ application.dnsType }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell class="align-top whitespace-normal break-all">
                    <div class="grid gap-1">
                      <div class="font-medium">
                        {{ application.name || application.primaryDomain }}
                      </div>
                      <div
                        class="font-mono text-xs text-muted-foreground break-all"
                      >
                        {{ application.domains.join(", ") }}
                      </div>
                      <div class="text-xs text-muted-foreground">
                        {{
                          application.renewEnabled
                            ? "已启用自动续期"
                            : "未启用自动续期"
                        }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell class="align-top whitespace-normal break-words">
                    <div class="grid gap-1">
                      <div class="flex flex-wrap gap-1">
                        <Badge :variant="certificateBadgeVariant(application)">
                          {{ certificateStatusLabel(application) }}
                        </Badge>
                        <Badge :variant="libraryBadgeVariant(application)">
                          {{ libraryStatusLabel(application) }}
                        </Badge>
                        <Badge
                          :variant="
                            jobBadgeVariant(application.latestJob?.status)
                          "
                        >
                          {{ latestJobLabel(application) }}
                        </Badge>
                      </div>
                      <div
                        v-if="application.certificate?.exists"
                        class="text-xs text-muted-foreground break-all"
                      >
                        {{ application.certificate?.issuer || "未知签发者" }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell
                    class="align-top whitespace-normal break-words text-xs leading-5 text-muted-foreground"
                  >
                    {{ formatCertificateRange(application) }}
                  </TableCell>

                  <TableCell class="align-top whitespace-normal text-right">
                    <div class="inline-flex">
                      <Button
                        type="button"
                        size="sm"
                        class="rounded-r-none"
                        :disabled="isActionBlocked()"
                        @click="requestCertificate(application.id)"
                      >
                        {{ primaryActionLabel(application) }}
                      </Button>
                      <DropdownMenu>
                        <DropdownMenuTrigger as-child>
                          <Button
                            type="button"
                            size="sm"
                            variant="default"
                            class="rounded-l-none border-l border-primary-foreground/20 px-2"
                            :disabled="isSecondaryActionDisabled(application)"
                          >
                            <ChevronDown class="h-4 w-4" />
                          </Button>
                        </DropdownMenuTrigger>
                        <DropdownMenuContent align="end" class="w-44">
                          <DropdownMenuItem
                            :disabled="isActionBlocked()"
                            @select="openEditDialog(application.id)"
                          >
                            编辑申请项
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            v-if="application.latestJob?.id"
                            @select="viewJob(application.latestJob.id)"
                          >
                            查看日志
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            v-if="application.certificate?.exists"
                            :disabled="isActionBlocked()"
                            @select="downloadCertificate(application)"
                          >
                            下载证书
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            v-if="application.certificate?.exists"
                            :disabled="isActionBlocked()"
                            @select="syncLibrary(application)"
                          >
                            {{
                              application.library?.linked
                                ? "更新到证书库"
                                : "添加到证书库"
                            }}
                          </DropdownMenuItem>
                          <DropdownMenuItem
                            v-if="application.certificate?.exists"
                            :disabled="isActionBlocked()"
                            @select="deployCertificate(application)"
                          >
                            设为当前证书
                          </DropdownMenuItem>
                          <DropdownMenuSeparator
                            v-if="application.certificate?.exists"
                          />
                          <DropdownMenuItem
                            v-if="application.certificate?.exists"
                            variant="destructive"
                            :disabled="isActionBlocked()"
                            @select="openDeleteDialog(application)"
                          >
                            删除证书
                          </DropdownMenuItem>
                        </DropdownMenuContent>
                      </DropdownMenu>
                    </div>
                  </TableCell>
                </TableRow>
              </TableBody>
            </Table>
          </div>
        </div>
      </CardContent>
    </Card>

    <AcmeJobPanel
      v-if="job"
      :job="job"
      :logs="logs"
      :analysis="analysis"
      :application-label="selectedApplicationLabel"
      :is-refreshing="isRefreshingLogs"
      @refresh="refreshLogs"
      @focus-credentials="focusCredentialsFromJob"
    />

    <AcmeApplicationDialog
      v-model:open="isDialogOpen"
      :mode="dialogMode"
      :initial-value="editingApplication"
      :dns-providers="dnsProviders"
      :pending="isDialogSubmitting"
      @submit="submitDialog"
    />

    <Dialog
      :open="Boolean(deleteCandidate)"
      @update:open="handleDeleteDialogOpenChange"
    >
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>确认删除 ACME 证书</DialogTitle>
          <DialogDescription class="leading-6">
            删除后会移除
            <span class="font-medium text-foreground">
              {{ deleteCandidateLabel || "当前申请项" }}
            </span>
            当前保存的证书和相关证书库关联，但会保留申请项配置。
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="isMutating"
            @click="closeDeleteDialog"
          >
            取消
          </Button>
          <Button
            type="button"
            variant="destructive"
            :disabled="isActionBlocked() || !deleteCandidate"
            @click="confirmDeleteCandidate"
          >
            确认删除
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import {
  AcmeAPI,
  type AcmeApplicationOverviewItem,
  type AcmeApplicationPayload,
  type AcmeApplicationRecord,
  type AcmeDnsProvider,
  type AcmeJobData,
  type AcmeLogAnalysis,
  type AcmeOverview,
} from "@/lib/api";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu";
import RefreshButton from "@/components/RefreshButton.vue";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@admin-shared/utils/toast";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import AcmeApplicationDialog from "./AcmeApplicationDialog.vue";
import AcmeJobPanel from "./AcmeJobPanel.vue";
import { ChevronDown } from "lucide-vue-next";

const overview = ref<AcmeOverview | null>(null);
const dnsProviders = ref<AcmeDnsProvider[]>([]);
const isDialogOpen = ref(false);
const dialogMode = ref<"create" | "edit">("create");
const editingApplication = ref<AcmeApplicationRecord | null>(null);
const deleteCandidate = ref<AcmeApplicationOverviewItem | null>(null);
const selectedJobId = ref("");
const job = ref<AcmeJobData | null>(null);
const logs = ref<string[]>([]);
const analysis = ref<AcmeLogAnalysis | null>(null);
let pollingTimer: ReturnType<typeof setInterval> | null = null;

const { isPending: isOverviewLoading, run: runLoadOverview } = useAsyncAction();
const { isPending: isProvidersLoading, run: runLoadProviders } =
  useAsyncAction();
const { isPending: isDialogSubmitting, run: runDialogSubmit } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "保存申请项失败"));
  },
});
const { isPending: isMutating, run: runMutating } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "操作失败"));
  },
});
const { isPending: isRefreshingLogs, run: runRefreshLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "刷新日志失败"));
  },
});
const { isPending: isDownloading, run: runDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "下载失败"));
  },
});
const { run: runLoadApplication } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, "加载申请项失败"));
  },
});

const applications = computed(() => overview.value?.applications || []);
const acmeState = computed(() => overview.value?.acmeState || null);
const isAcmeInstalled = computed(() => acmeState.value?.status === "installed");
const isTableLocked = computed(() => overview.value?.lock.locked === true);
const lockedApplication = computed(() => {
  const applicationId = overview.value?.lock.applicationId;
  if (!applicationId) return null;
  return applications.value.find((item) => item.id === applicationId) || null;
});

const acmeStatusLabel = computed(() => {
  const status = acmeState.value?.status;
  if (status === "installed") return "已就绪";
  if (status === "installing") return "安装中";
  if (status === "error") return "异常";
  return "未安装";
});

const acmeStatusBadgeVariant = computed(() => {
  const status = acmeState.value?.status;
  if (status === "installed") return "secondary";
  if (status === "error") return "destructive";
  if (status === "installing") return "default";
  return "outline";
});

const lockReasonLabel = computed(() => {
  const reason = overview.value?.lock.reason;
  return reason === "auto_renew" ? "自动续期中" : "任务执行中";
});

const lockMessageTitle = computed(() => {
  const target =
    lockedApplication.value?.name || lockedApplication.value?.primaryDomain;
  if (!target) {
    return overview.value?.lock.reason === "auto_renew"
      ? "正在自动续期证书"
      : "正在申请证书";
  }
  return overview.value?.lock.reason === "auto_renew"
    ? `正在为 ${target} 自动续期证书`
    : `正在为 ${target} 申请证书`;
});

const lockMessageDescription = computed(() => {
  return "任务执行期间会锁定列表操作，你仍然可以查看列表内容和下方日志。";
});

const selectedApplicationLabel = computed(() => {
  const applicationId = job.value?.applicationId;
  if (!applicationId) return "";
  const application = applications.value.find(
    (item) => item.id === applicationId,
  );
  return application?.name || application?.primaryDomain || "";
});

const deleteCandidateLabel = computed(() => {
  return (
    deleteCandidate.value?.name || deleteCandidate.value?.primaryDomain || ""
  );
});

const refresh = async () => {
  await Promise.all([fetchOverview(), loadProviders()]);
};

const fetchOverview = async (opts?: {
  silent?: boolean;
  preserveSelection?: boolean;
}) => {
  await runLoadOverview(
    async () => {
      const data = await AcmeAPI.overview();
      overview.value = data;

      const runningJobId = data.runningJob?.id || data.lock.jobId || "";
      if (runningJobId) {
        await selectJob(runningJobId, true);
        return;
      }

      if (!opts?.preserveSelection && !selectedJobId.value) {
        const latestFailedJob = data.applications.find(
          (application) => application.latestJob?.status === "failed",
        );
        if (latestFailedJob?.latestJob?.id) {
          await selectJob(latestFailedJob.latestJob.id, false);
        }
      }
    },
    {
      onError: (error) => {
        if (!opts?.silent) {
          toast.error(extractErrorMessage(error, "加载 ACME 概览失败"));
        }
      },
    },
  );
};

const loadProviders = async () => {
  await runLoadProviders(
    async () => {
      dnsProviders.value = await AcmeAPI.dnsProviders();
    },
    {
      onError: (error) => {
        toast.error(extractErrorMessage(error, "加载 DNS 服务商失败"));
        dnsProviders.value = [];
      },
    },
  );
};

const stopPolling = () => {
  if (pollingTimer) {
    clearInterval(pollingTimer);
    pollingTimer = null;
  }
};

const pollJobOnce = async (jobId: string) => {
  const data = await AcmeAPI.poll(jobId, { limit: 500, order: "desc" });
  job.value = data.job;
  logs.value = data.logs;
  analysis.value = data.analysis ?? null;

  if (data.job.status === "succeeded" || data.job.status === "failed") {
    stopPolling();
    await fetchOverview({ silent: true, preserveSelection: true });
  }
};

const startPolling = (jobId: string) => {
  stopPolling();
  pollingTimer = setInterval(async () => {
    try {
      await pollJobOnce(jobId);
    } catch {
      // Ignore polling errors and keep the last visible state.
    }
  }, 2000);
};

const selectJob = async (jobId: string, autoPoll: boolean) => {
  if (!jobId) return;
  selectedJobId.value = jobId;
  await pollJobOnce(jobId);
  if (
    autoPoll &&
    (job.value?.status === "queued" || job.value?.status === "running")
  ) {
    startPolling(jobId);
  } else {
    stopPolling();
  }
};

const viewJob = async (jobId: string) => {
  await selectJob(jobId, false);
};

const openCreateDialog = () => {
  dialogMode.value = "create";
  editingApplication.value = null;
  isDialogOpen.value = true;
};

const openEditDialog = async (applicationId: string) => {
  await runLoadApplication(async () => {
    const application = await AcmeAPI.getApplication(applicationId);
    dialogMode.value = "edit";
    editingApplication.value = application;
    isDialogOpen.value = true;
  });
};

const openDeleteDialog = (application: AcmeApplicationOverviewItem) => {
  deleteCandidate.value = application;
};

const closeDeleteDialog = () => {
  if (isMutating.value) return;
  deleteCandidate.value = null;
};

const handleDeleteDialogOpenChange = (open: boolean) => {
  if (!open) {
    closeDeleteDialog();
  }
};

const submitDialog = async (payload: AcmeApplicationPayload) => {
  await runDialogSubmit(async () => {
    const response =
      dialogMode.value === "edit" && editingApplication.value
        ? await AcmeAPI.updateApplication(editingApplication.value.id, payload)
        : await AcmeAPI.createApplication(payload);

    toast.success(payload.submitNow ? "任务已提交" : "已保存");
    isDialogOpen.value = false;
    editingApplication.value = null;
    await fetchOverview({ silent: true, preserveSelection: true });

    if (response.job?.id) {
      await selectJob(response.job.id, true);
    }
  });
};

const requestCertificate = async (applicationId: string) => {
  await runMutating(async () => {
    const response = await AcmeAPI.requestApplication(applicationId);
    toast.success("任务已提交");
    await fetchOverview({ silent: true, preserveSelection: true });
    await selectJob(response.job.id, true);
  });
};

const syncLibrary = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.syncApplicationLibrary(application.id);
    toast.success(
      application.library?.linked ? "已更新到证书库" : "已添加到证书库",
    );
    await fetchOverview({ silent: true, preserveSelection: true });
  });
};

const deployCertificate = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.deployApplication(application.id);
    toast.success("已设为当前证书");
    await fetchOverview({ silent: true, preserveSelection: true });
  });
};

const deleteCertificate = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.deleteApplicationCertificate(application.id);
    toast.success("已删除证书");
    await fetchOverview({ silent: true, preserveSelection: true });
    if (
      job.value?.applicationId === application.id &&
      job.value.status !== "running"
    ) {
      job.value = null;
      logs.value = [];
      analysis.value = null;
      selectedJobId.value = "";
    }
  });
};

const confirmDeleteCandidate = async () => {
  if (!deleteCandidate.value) return;
  const application = deleteCandidate.value;
  await deleteCertificate(application);
  if (deleteCandidate.value?.id === application.id) {
    deleteCandidate.value = null;
  }
};

const downloadCertificate = async (
  application: AcmeApplicationOverviewItem,
) => {
  await runDownload(async () => {
    const blob = await AcmeAPI.download(application.primaryDomain);
    downloadBlob(blob, `${application.primaryDomain}.zip`);
  });
};

const refreshLogs = async () => {
  if (!selectedJobId.value) return;
  await runRefreshLogs(() => pollJobOnce(selectedJobId.value));
};

const focusCredentialsFromJob = async () => {
  const applicationId = job.value?.applicationId;
  if (!applicationId) return;
  await openEditDialog(applicationId);
};

const isActionBlocked = () => {
  if (!isAcmeInstalled.value) return true;
  if (isTableLocked.value) return true;
  if (isMutating.value || isDialogSubmitting.value || isDownloading.value) {
    return true;
  }
  return false;
};

const primaryActionLabel = (application: AcmeApplicationOverviewItem) => {
  return application.certificate?.exists ? "重申请" : "申请";
};

const isSecondaryActionDisabled = (
  application: AcmeApplicationOverviewItem,
) => {
  return isActionBlocked() && !application.latestJob?.id;
};

const certificateStatusLabel = (application: AcmeApplicationOverviewItem) => {
  if (!application.certificate?.exists) return "无证书";
  const validTo = Date.parse(application.certificate.validTo || "");
  if (!Number.isFinite(validTo)) return "证书异常";
  if (validTo <= Date.now()) return "已过期";
  if (validTo - Date.now() <= 30 * 24 * 60 * 60 * 1000) return "即将过期";
  return "有证书";
};

const certificateBadgeVariant = (application: AcmeApplicationOverviewItem) => {
  const label = certificateStatusLabel(application);
  if (label === "无证书") return "outline";
  if (label === "有证书") return "secondary";
  return "destructive";
};

const formatCertificateRange = (application: AcmeApplicationOverviewItem) => {
  if (!application.certificate?.exists) return "未签发";
  const validFrom = application.certificate.validFrom || "";
  const validTo = application.certificate.validTo || "";
  if (!validFrom || !validTo) return "证书信息异常";
  return `${formatDate(validFrom)} ~ ${formatDate(validTo)}`;
};

const latestJobLabel = (application: AcmeApplicationOverviewItem) => {
  const status = application.latestJob?.status;
  if (!status || status === "idle") return "未运行";
  if (status === "queued") return "排队中";
  if (status === "running") return "执行中";
  if (status === "succeeded") return "最近成功";
  if (status === "failed") return "最近失败";
  return status;
};

const jobBadgeVariant = (status?: string | null) => {
  if (!status || status === "idle") return "outline";
  if (status === "queued") return "outline";
  if (status === "running") return "default";
  if (status === "succeeded") return "secondary";
  return "destructive";
};

const libraryStatusLabel = (application: AcmeApplicationOverviewItem) => {
  if (application.library?.isActive) return "当前使用中";
  if (application.library?.linked) return "已加入";
  return "未加入";
};

const libraryBadgeVariant = (application: AcmeApplicationOverviewItem) => {
  if (application.library?.isActive) return "default";
  if (application.library?.linked) return "secondary";
  return "outline";
};

const formatDate = (value: string) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString();
};

onMounted(async () => {
  await Promise.all([fetchOverview({ silent: true }), loadProviders()]);
});

onUnmounted(() => {
  stopPolling();
});
</script>
