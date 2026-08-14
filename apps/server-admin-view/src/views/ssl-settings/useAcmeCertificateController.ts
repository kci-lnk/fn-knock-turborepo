import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  AcmeAPI,
  type AcmeApplicationOverviewItem,
  type AcmeApplicationPayload,
  type AcmeApplicationRecord,
  type AcmeDnsProvider,
  type AcmeOverview,
} from "@/lib/api/acme";
import { acmeCertificateArchiveFilename } from "@/lib/acme-download";
import { useConfigStore } from "@/store/config";
import { toast } from "@admin-shared/utils/toast";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useAcmeCertificateDisplay } from "./useAcmeCertificateDisplay";
import { useAcmeJobPolling } from "./useAcmeJobPolling";

export function useAcmeCertificateController() {
  const { t } = useI18n();
  const router = useRouter();
  const configStore = useConfigStore();
  const overview = ref<AcmeOverview | null>(null);
  const dnsProviders = ref<AcmeDnsProvider[]>([]);
  const isDialogOpen = ref(false);
  const dialogMode = ref<"create" | "edit">("create");
  const editingApplication = ref<AcmeApplicationRecord | null>(null);
  const deleteCandidate = ref<AcmeApplicationOverviewItem | null>(null);
  const deletingApplicationId = ref("");

  const { isPending: isOverviewLoading, run: runLoadOverview } =
    useAsyncAction();
  const { isPending: isProvidersLoading, run: runLoadProviders } =
    useAsyncAction();
  const { isPending: isDialogSubmitting, run: runDialogSubmit } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(error, t("admin.acmeCert.saveApplicationFailed")),
        );
      },
    });
  const { isPending: isMutating, run: runMutating } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.acmeCert.operationFailed")),
      );
    },
  });
  const { isPending: isDownloading, run: runDownload } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.acmeCert.downloadFailed")),
      );
    },
  });
  const { run: runLoadApplication } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.acmeCert.loadApplicationFailed")),
      );
    },
  });

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
              extractErrorMessage(
                error,
                t("admin.acmeCert.loadOverviewFailed"),
              ),
            );
          }
        },
      },
    );
  };

  const {
    analysis,
    clearSelectedJob,
    isRefreshingLogs,
    isStoppingJob,
    job,
    logs,
    refreshLogs,
    selectJob,
    selectedJobId,
    stopActiveJob,
    viewJob,
  } = useAcmeJobPolling({
    refreshOverview: () =>
      fetchOverview({ silent: true, preserveSelection: true }),
  });

  const applications = computed(() => overview.value?.applications || []);
  const acmeState = computed(() => overview.value?.acmeState || null);
  const isAcmeInstalled = computed(
    () => acmeState.value?.status === "installed",
  );
  const shouldPromptAcmeInitialization = computed(
    () => acmeState.value?.status === "uninstalled",
  );
  const isTableLocked = computed(() => overview.value?.lock.locked === true);
  const canStopActiveJob = computed(() => {
    if (!isTableLocked.value) return false;
    if (isStoppingJob.value) return true;
    return Boolean(
      overview.value?.lock.jobId || job.value?.status === "running",
    );
  });
  const lockedApplication = computed(() => {
    const applicationId = overview.value?.lock.applicationId;
    if (!applicationId) return null;
    return applications.value.find((item) => item.id === applicationId) || null;
  });

  const acmeStatusLabel = computed(() => {
    const status = acmeState.value?.status;
    if (status === "installed") return t("admin.acmeCert.acmeStatus.ready");
    if (status === "installing") {
      return t("admin.acmeCert.acmeStatus.installing");
    }
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

  const lockReasonLabel = computed(() =>
    overview.value?.lock.reason === "auto_renew"
      ? t("admin.acmeCert.lock.autoRenew")
      : t("admin.acmeCert.lock.running"),
  );

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

  const lockMessageDescription = computed(() =>
    t("admin.acmeCert.lock.description"),
  );

  const selectedApplicationLabel = computed(() => {
    const applicationId = job.value?.applicationId;
    if (!applicationId) return "";
    const application = applications.value.find(
      (item) => item.id === applicationId,
    );
    return application?.name || application?.primaryDomain || "";
  });

  const deleteCandidateLabel = computed(
    () =>
      deleteCandidate.value?.name || deleteCandidate.value?.primaryDomain || "",
  );

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

  const refresh = async () => {
    await Promise.all([fetchOverview(), loadProviders()]);
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
    if (!open) closeDeleteDialog();
  };

  const submitDialog = async (payload: AcmeApplicationPayload) => {
    await runDialogSubmit(async () => {
      const response =
        dialogMode.value === "edit" && editingApplication.value
          ? await AcmeAPI.updateApplication(
              editingApplication.value.id,
              payload,
            )
          : await AcmeAPI.createApplication(payload);

      toast.success(
        payload.submitNow
          ? t("admin.acmeCert.taskSubmitted")
          : t("admin.acmeCert.saved"),
      );
      isDialogOpen.value = false;
      editingApplication.value = null;
      await fetchOverview({ silent: true, preserveSelection: true });

      if (response.job?.id) await selectJob(response.job.id, true);
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

  const deployCertificate = async (
    application: AcmeApplicationOverviewItem,
  ) => {
    await runMutating(async () => {
      await AcmeAPI.deployApplication(application.id);
      toast.success(t("admin.acmeCert.deployed"));
      await fetchOverview({ silent: true, preserveSelection: true });
    });
  };

  const deleteCertificate = async (
    application: AcmeApplicationOverviewItem,
  ) => {
    await runMutating(async () => {
      await AcmeAPI.deleteApplicationCertificate(application.id);
      toast.success(t("admin.acmeCert.certificateDeleted"));
      await fetchOverview({ silent: true, preserveSelection: true });
      clearSelectedJob(application.id, { includeRunning: false });
    });
  };

  const removeApplication = async (
    application: AcmeApplicationOverviewItem,
  ) => {
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

        clearSelectedJob(application.id);
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
      downloadBlob(
        blob,
        acmeCertificateArchiveFilename(application.primaryDomain),
      );
    });
  };

  const focusCredentialsFromJob = async () => {
    const applicationId = job.value?.applicationId;
    if (applicationId) await openEditDialog(applicationId);
  };

  const isActionBlocked = () =>
    !isAcmeInstalled.value ||
    isTableLocked.value ||
    isMutating.value ||
    isDialogSubmitting.value ||
    isDownloading.value ||
    isStoppingJob.value;

  const isConfigurationEditBlocked = () =>
    isMutating.value ||
    isDialogSubmitting.value ||
    isDownloading.value ||
    isStoppingJob.value;

  const isDeleteApplicationBlocked = () =>
    isTableLocked.value ||
    isMutating.value ||
    isDialogSubmitting.value ||
    isDownloading.value ||
    isStoppingJob.value;

  const {
    certificateBadgeVariant,
    certificateStatusLabel,
    deleteApplicationDescription,
    formatCertificateRange,
    isSecondaryActionDisabled,
    jobBadgeVariant,
    latestJobLabel,
    libraryBadgeVariant,
    libraryStatusLabel,
    primaryActionLabel,
  } = useAcmeCertificateDisplay({ isConfigurationEditBlocked });

  onMounted(async () => {
    await Promise.all([fetchOverview({ silent: true }), loadProviders()]);
  });

  return {
    acmeStatusBadgeVariant,
    acmeStatusLabel,
    analysis,
    applications,
    canStopActiveJob,
    certificateBadgeVariant,
    certificateStatusLabel,
    closeDeleteDialog,
    configStore,
    confirmDeleteCandidate,
    deleteApplicationDescription,
    deleteCandidate,
    deleteCandidateLabel,
    deletingApplicationId,
    deployCertificate,
    dialogMode,
    dnsProviders,
    downloadCertificate,
    editingApplication,
    focusCredentialsFromJob,
    formatCertificateRange,
    goToAcmeInitialization,
    handleDeleteDialogOpenChange,
    isAcmeInstalled,
    isActionBlocked,
    isConfigurationEditBlocked,
    isDeleteApplicationBlocked,
    isDialogOpen,
    isDialogSubmitting,
    isMutating,
    isOverviewLoading,
    isProvidersLoading,
    isRefreshingLogs,
    isSecondaryActionDisabled,
    isStoppingJob,
    isTableLocked,
    job,
    jobBadgeVariant,
    latestJobLabel,
    libraryBadgeVariant,
    libraryStatusLabel,
    lockMessageDescription,
    lockMessageTitle,
    lockReasonLabel,
    logs,
    openCreateDialog,
    openDeleteDialog,
    openEditDialog,
    primaryActionLabel,
    refresh,
    refreshLogs,
    removeApplication,
    requestCertificate,
    selectedApplicationLabel,
    shouldPromptAcmeInitialization,
    stopActiveJob,
    submitDialog,
    syncLibrary,
    t,
    viewJob,
  };
}
