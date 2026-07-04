<template>
  <div class="grid gap-4">
    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <div
          class="flex flex-col gap-4 lg:flex-row lg:items-start lg:justify-between"
        >
          <div class="grid gap-1">
            <CardTitle class="flex flex-wrap items-center gap-2">
              {{ t("admin.acmeCert.title") }}
              <Badge :variant="acmeStatusBadgeVariant">{{
                acmeStatusLabel
              }}</Badge>
              <Badge v-if="isTableLocked" variant="outline">
                {{ lockReasonLabel }}
              </Badge>
            </CardTitle>
            <CardDescription>
              {{ t("admin.acmeCert.description") }}
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
                !isAcmeInstalled ||
                isTableLocked ||
                isDialogSubmitting ||
                !dnsProviders.length
              "
              @click="openCreateDialog"
            >
              {{ t("admin.acmeCert.newApplication") }}
            </Button>
          </div>
        </div>
      </CardHeader>
    </Card>

    <Alert
      v-if="shouldPromptAcmeInitialization"
      class="border-amber-200 bg-amber-50 text-amber-950 dark:border-amber-900/50 dark:bg-amber-950/20 dark:text-amber-100"
    >
      <AlertTriangle class="h-4 w-4" />
      <AlertTitle>
        {{ t("admin.acmeCert.initializePromptTitle") }}
      </AlertTitle>
      <AlertDescription
        class="grid gap-3 text-amber-900 sm:grid-cols-[1fr_auto] sm:items-center dark:text-amber-100/90"
      >
        <span>{{ t("admin.acmeCert.initializePromptDescription") }}</span>
        <Button
          type="button"
          size="sm"
          variant="outline"
          class="shrink-0 border-amber-300 bg-background/80 text-amber-950 hover:bg-background dark:border-amber-700 dark:text-amber-100"
          @click="goToAcmeInitialization"
        >
          {{ t("admin.acmeCert.goInitialize") }}
        </Button>
      </AlertDescription>
    </Alert>

    <Card class="border-border/80 shadow-sm">
      <CardHeader>
        <CardTitle>{{ t("admin.acmeCert.applicationList") }}</CardTitle>
        <CardDescription>
          {{ t("admin.acmeCert.applicationListDescription") }}
        </CardDescription>
      </CardHeader>
      <CardContent>
        <div class="relative">
          <div
            v-if="isTableLocked"
            class="absolute inset-0 z-10 flex items-center justify-center rounded-lg border bg-background/80 p-4 backdrop-blur-sm"
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
                    {{ t("admin.acmeCert.dnsProvider") }}
                  </TableHead>
                  <TableHead class="w-[120px] whitespace-normal">
                    {{ t("admin.acmeCert.domain") }}
                  </TableHead>
                  <TableHead class="w-[180px] whitespace-normal">
                    {{ t("admin.acmeCert.statusOverview") }}
                  </TableHead>
                  <TableHead class="w-[150px] whitespace-normal"
                    >{{ t("admin.acmeCert.validity") }}</TableHead
                  >
                  <TableHead class="w-[156px] whitespace-normal text-right">
                    {{ t("admin.acmeCert.actions") }}
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
                    {{
                      shouldPromptAcmeInitialization
                        ? t("admin.acmeCert.emptyApplicationsBeforeInit")
                        : t("admin.acmeCert.emptyApplications")
                    }}
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
                            ? t("admin.acmeCert.autoRenewEnabled")
                            : t("admin.acmeCert.autoRenewDisabled")
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
                        {{
                          application.certificate?.issuer ||
                          t("admin.acmeCert.unknownIssuer")
                        }}
                      </div>
                    </div>
                  </TableCell>

                  <TableCell
                    class="align-top whitespace-normal break-words text-xs leading-5 text-muted-foreground"
                  >
                    {{ formatCertificateRange(application) }}
                  </TableCell>

                  <TableCell class="align-top whitespace-normal text-right">
                    <div class="inline-flex items-center gap-2">
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
                              {{ t("admin.acmeCert.editApplication") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.latestJob?.id"
                              @select="viewJob(application.latestJob.id)"
                            >
                              {{ t("admin.acmeCert.viewLogs") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="downloadCertificate(application)"
                            >
                              {{ t("admin.acmeCert.downloadCertificate") }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="syncLibrary(application)"
                            >
                              {{
                                application.library?.linked
                                  ? t("admin.acmeCert.updateToLibrary")
                                  : t("admin.acmeCert.addToLibrary")
                              }}
                            </DropdownMenuItem>
                            <DropdownMenuItem
                              v-if="application.certificate?.exists"
                              :disabled="isActionBlocked()"
                              @select="deployCertificate(application)"
                            >
                              {{ t("admin.acmeCert.setAsCurrentCertificate") }}
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
                              {{ t("admin.acmeCert.deleteCertificate") }}
                            </DropdownMenuItem>
                          </DropdownMenuContent>
                        </DropdownMenu>
                      </div>

                      <ConfirmDangerPopover
                        :title="t('admin.acmeCert.confirmDeleteApplicationTitle')"
                        :description="deleteApplicationDescription(application)"
                        :confirm-text="t('admin.acmeCert.deleteApplication')"
                        :loading="deletingApplicationId === application.id"
                        :disabled="isDeleteApplicationBlocked()"
                        :on-confirm="() => removeApplication(application)"
                        content-class="w-80 text-left"
                      >
                        <template #trigger>
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            class="h-8 w-8 text-destructive hover:bg-destructive/10 hover:text-destructive"
                            :disabled="isDeleteApplicationBlocked()"
                          >
                            <Trash2 class="h-4 w-4" />
                          </Button>
                        </template>
                      </ConfirmDangerPopover>
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
      :can-stop="canStopActiveJob"
      :is-stopping="isStoppingJob"
      :stop-action="stopActiveJob"
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
          <DialogTitle>
            {{ t("admin.acmeCert.confirmDeleteCertificateTitle") }}
          </DialogTitle>
          <DialogDescription class="leading-6">
            {{
              t("admin.acmeCert.confirmDeleteCertificateDescription", {
                target:
                  deleteCandidateLabel || t("admin.acmeCert.currentApplication"),
              })
            }}
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            :disabled="isMutating"
            @click="closeDeleteDialog"
          >
            {{ t("common.cancel") }}
          </Button>
          <Button
            type="button"
            variant="destructive"
            :disabled="isActionBlocked() || !deleteCandidate"
            @click="confirmDeleteCandidate"
          >
            {{ t("common.confirmDelete") }}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
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
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import AcmeApplicationDialog from "./AcmeApplicationDialog.vue";
import AcmeJobPanel from "./AcmeJobPanel.vue";
import { AlertTriangle, ChevronDown, Trash2 } from "lucide-vue-next";

type CertificateStatusKey =
  | "none"
  | "invalid"
  | "expired"
  | "expiring"
  | "valid";

const { locale, t } = useI18n();
const router = useRouter();
const overview = ref<AcmeOverview | null>(null);
const dnsProviders = ref<AcmeDnsProvider[]>([]);
const isDialogOpen = ref(false);
const dialogMode = ref<"create" | "edit">("create");
const editingApplication = ref<AcmeApplicationRecord | null>(null);
const deleteCandidate = ref<AcmeApplicationOverviewItem | null>(null);
const deletingApplicationId = ref("");
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
    toast.error(
      extractErrorMessage(error, t("admin.acmeCert.saveApplicationFailed")),
    );
  },
});
const { isPending: isMutating, run: runMutating } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeCert.operationFailed")));
  },
});
const { isPending: isRefreshingLogs, run: runRefreshLogs } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeCert.refreshLogsFailed")));
  },
});
const { isPending: isStoppingJob, run: runStopJob } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeCert.stopJobFailed")));
  },
});
const { isPending: isDownloading, run: runDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t("admin.acmeCert.downloadFailed")));
  },
});
const { run: runLoadApplication } = useAsyncAction({
  onError: (error) => {
    toast.error(
      extractErrorMessage(error, t("admin.acmeCert.loadApplicationFailed")),
    );
  },
});

const applications = computed(() => overview.value?.applications || []);
const acmeState = computed(() => overview.value?.acmeState || null);
const isAcmeInstalled = computed(() => acmeState.value?.status === "installed");
const shouldPromptAcmeInitialization = computed(
  () => acmeState.value?.status === "uninstalled",
);
const isTableLocked = computed(() => overview.value?.lock.locked === true);
const canStopActiveJob = computed(() => {
  if (!isTableLocked.value) return false;
  if (isStoppingJob.value) return true;
  return Boolean(overview.value?.lock.jobId || job.value?.status === "running");
});
const lockedApplication = computed(() => {
  const applicationId = overview.value?.lock.applicationId;
  if (!applicationId) return null;
  return applications.value.find((item) => item.id === applicationId) || null;
});

const acmeStatusLabel = computed(() => {
  const status = acmeState.value?.status;
  if (status === "installed") return t("admin.acmeCert.acmeStatus.ready");
  if (status === "installing") return t("admin.acmeCert.acmeStatus.installing");
  if (status === "error") return t("admin.acmeCert.acmeStatus.error");
  return t("admin.acmeCert.acmeStatus.notInstalled");
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
  return reason === "auto_renew"
    ? t("admin.acmeCert.lock.autoRenew")
    : t("admin.acmeCert.lock.running");
});

const lockMessageTitle = computed(() => {
  const target =
    lockedApplication.value?.name || lockedApplication.value?.primaryDomain;
  if (!target) {
    return overview.value?.lock.reason === "auto_renew"
      ? t("admin.acmeCert.lock.autoRenewTitle")
      : t("admin.acmeCert.lock.requestTitle");
  }
  return overview.value?.lock.reason === "auto_renew"
    ? t("admin.acmeCert.lock.autoRenewFor", { target })
    : t("admin.acmeCert.lock.requestFor", { target });
});

const lockMessageDescription = computed(() => {
  return t("admin.acmeCert.lock.description");
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
          toast.error(
            extractErrorMessage(error, t("admin.acmeCert.loadOverviewFailed")),
          );
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
        toast.error(
          extractErrorMessage(error, t("admin.acmeCert.loadProvidersFailed")),
        );
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

  if (
    data.job.status === "succeeded" ||
    data.job.status === "failed" ||
    data.job.status === "stopped"
  ) {
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

const goToAcmeInitialization = () => {
  void router.push({ path: "/system", query: { tab: "acme-ssl" } });
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

    toast.success(
      payload.submitNow
        ? t("admin.acmeCert.taskSubmitted")
        : t("admin.acmeCert.saved"),
    );
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
    toast.success(t("admin.acmeCert.taskSubmitted"));
    await fetchOverview({ silent: true, preserveSelection: true });
    await selectJob(response.job.id, true);
  });
};

const syncLibrary = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.syncApplicationLibrary(application.id);
    toast.success(
      application.library?.linked
        ? t("admin.acmeCert.updatedToLibrary")
        : t("admin.acmeCert.addedToLibrary"),
    );
    await fetchOverview({ silent: true, preserveSelection: true });
  });
};

const deployCertificate = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.deployApplication(application.id);
    toast.success(t("admin.acmeCert.deployed"));
    await fetchOverview({ silent: true, preserveSelection: true });
  });
};

const deleteCertificate = async (application: AcmeApplicationOverviewItem) => {
  await runMutating(async () => {
    await AcmeAPI.deleteApplicationCertificate(application.id);
    toast.success(t("admin.acmeCert.certificateDeleted"));
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

const removeApplication = async (application: AcmeApplicationOverviewItem) => {
  deletingApplicationId.value = application.id;
  try {
    await runMutating(async () => {
      await AcmeAPI.deleteApplication(application.id);
      toast.success(t("admin.acmeCert.applicationDeleted"));
      await fetchOverview({ silent: true, preserveSelection: true });

      if (editingApplication.value?.id === application.id) {
        editingApplication.value = null;
        isDialogOpen.value = false;
      }

      if (job.value?.applicationId === application.id) {
        job.value = null;
        logs.value = [];
        analysis.value = null;
        selectedJobId.value = "";
      }
    });
  } finally {
    if (deletingApplicationId.value === application.id) {
      deletingApplicationId.value = "";
    }
  }
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

const stopActiveJob = async () => {
  await runStopJob(async () => {
    const result = await AcmeAPI.stopActiveJob();
    stopPolling();
    const killedCount =
      result.processResult.matchedPids.length -
      result.processResult.remainingPids.length;
    if (result.stopped) {
      toast.success(t("admin.acmeCert.jobStopped"), {
        description:
          result.processResult.matchedPids.length > 0
            ? t("admin.acmeCert.jobStoppedDescription", {
                count: Math.max(0, killedCount),
              })
            : t("admin.acmeCert.noRunningProcesses"),
      });
    } else {
      toast.info(t("admin.acmeCert.noActiveJob"));
    }

    await fetchOverview({ silent: true, preserveSelection: true });
    const stoppedJobId = result.job?.id || selectedJobId.value;
    if (stoppedJobId) {
      await pollJobOnce(stoppedJobId);
    }
  });
};

const focusCredentialsFromJob = async () => {
  const applicationId = job.value?.applicationId;
  if (!applicationId) return;
  await openEditDialog(applicationId);
};

const isActionBlocked = () => {
  if (!isAcmeInstalled.value) return true;
  if (isTableLocked.value) return true;
  if (
    isMutating.value ||
    isDialogSubmitting.value ||
    isDownloading.value ||
    isStoppingJob.value
  ) {
    return true;
  }
  return false;
};

const isDeleteApplicationBlocked = () => {
  if (isTableLocked.value) return true;
  if (
    isMutating.value ||
    isDialogSubmitting.value ||
    isDownloading.value ||
    isStoppingJob.value
  ) {
    return true;
  }
  return false;
};

const primaryActionLabel = (application: AcmeApplicationOverviewItem) => {
  return application.certificate?.exists
    ? t("admin.acmeCert.reapply")
    : t("admin.acmeCert.apply");
};

const isSecondaryActionDisabled = (
  application: AcmeApplicationOverviewItem,
) => {
  return isActionBlocked() && !application.latestJob?.id;
};

const certificateStatusKey = (
  application: AcmeApplicationOverviewItem,
): CertificateStatusKey => {
  if (!application.certificate?.exists) return "none";
  const validTo = Date.parse(application.certificate.validTo || "");
  if (!Number.isFinite(validTo)) return "invalid";
  if (validTo <= Date.now()) return "expired";
  if (validTo - Date.now() <= 30 * 24 * 60 * 60 * 1000) return "expiring";
  return "valid";
};

const certificateStatusLabel = (application: AcmeApplicationOverviewItem) => {
  const key = certificateStatusKey(application);
  return t(`admin.acmeCert.certificateStatus.${key}`);
};

const certificateBadgeVariant = (application: AcmeApplicationOverviewItem) => {
  const key = certificateStatusKey(application);
  if (key === "none") return "outline";
  if (key === "valid") return "secondary";
  return "destructive";
};

const formatCertificateRange = (application: AcmeApplicationOverviewItem) => {
  if (!application.certificate?.exists) return t("admin.acmeCert.notIssued");
  const validFrom = application.certificate.validFrom || "";
  const validTo = application.certificate.validTo || "";
  if (!validFrom || !validTo) {
    return t("admin.acmeCert.certificateInfoInvalid");
  }
  return `${formatDate(validFrom)} ~ ${formatDate(validTo)}`;
};

const latestJobLabel = (application: AcmeApplicationOverviewItem) => {
  const status = application.latestJob?.status;
  if (!status || status === "idle") return t("admin.acmeCert.jobStatus.idle");
  if (status === "queued") return t("admin.acmeCert.jobStatus.queued");
  if (status === "running") return t("admin.acmeCert.jobStatus.running");
  if (status === "succeeded") return t("admin.acmeCert.jobStatus.succeeded");
  if (status === "failed") return t("admin.acmeCert.jobStatus.failed");
  if (status === "stopped") return t("admin.acmeCert.jobStatus.stopped");
  return status;
};

const jobBadgeVariant = (status?: string | null) => {
  if (!status || status === "idle") return "outline";
  if (status === "queued") return "outline";
  if (status === "running") return "default";
  if (status === "succeeded") return "secondary";
  if (status === "failed") return "outline";
  if (status === "stopped") return "outline";
  return "outline";
};

const libraryStatusLabel = (application: AcmeApplicationOverviewItem) => {
  if (application.library?.isActive) return t("admin.acmeCert.library.active");
  if (application.library?.linked) return t("admin.acmeCert.library.linked");
  return t("admin.acmeCert.library.unlinked");
};

const libraryBadgeVariant = (application: AcmeApplicationOverviewItem) => {
  if (application.library?.isActive) return "default";
  if (application.library?.linked) return "secondary";
  return "outline";
};

const deleteApplicationDescription = (
  application: AcmeApplicationOverviewItem,
) => {
  const target = application.name || application.primaryDomain;
  if (application.certificate?.exists || application.library?.linked) {
    return t("admin.acmeCert.deleteApplicationWithCertificateDescription", {
      target,
    });
  }
  return t("admin.acmeCert.deleteApplicationDescription", { target });
};

const formatDate = (value: string) => {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return date.toLocaleString(locale.value);
};

onMounted(async () => {
  await Promise.all([fetchOverview({ silent: true }), loadProviders()]);
});

onUnmounted(() => {
  stopPolling();
});
</script>
