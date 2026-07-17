import { computed, onMounted, ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

export const useSelfSignedCA = ({
  locale,
  translate,
}: {
  locale: Ref<string>;
  translate: Translate;
}) => {
  const newHost = ref("");
  const hosts = ref<string[]>([]);
  const parseHosts = (value: string) =>
    value
      .split(/[，,]/gu)
      .map((item) => item.trim())
      .filter(Boolean);
  const pendingHosts = computed(() => [...new Set(parseHosts(newHost.value))]);

  const hasRootCA = ref(false);
  const caInfo = ref<{
    subject: string;
    issuer: string;
    validFrom: string;
    validTo: string;
    serialNumber: string;
  } | null>(null);
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
      toast.error(translate("admin.selfSignedCA.clearFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.selfSignedCA.unknownError"),
        ),
      });
    },
  });
  const { isPending: isRegenerating, run: runRegenerateRootCA } =
    useAsyncAction({
      onError: (error) => {
        toast.error(translate("admin.selfSignedCA.regenerateFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.selfSignedCA.unknownError"),
          ),
        });
      },
    });
  const { isPending: isDownloading, run: runDownloadFile } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          translate("admin.selfSignedCA.downloadFailed"),
        ),
      );
    },
  });
  const { run: runRefreshCAStatus } = useAsyncAction({
    onError: () => {
      hasRootCA.value = false;
      caInfo.value = null;
      hosts.value = [];
    },
  });

  const isIP = (value: string) => {
    const normalized = value.trim();
    const withoutPort = normalized.includes(":")
      ? normalized.split(":")[0] || normalized
      : normalized;
    return /^(?:(?:25[0-5]|2[0-4]\d|[01]?\d?\d)(?:\.|$)){4}$/u.test(
      withoutPort,
    );
  };

  const refreshCAStatus = async () => {
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
  };

  const addHost = async () => {
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
          newHost.value = "";
          toast.success(
            entries.length > 1
              ? translate("admin.selfSignedCA.hostsAdded", {
                  count: entries.length,
                })
              : translate("admin.selfSignedCA.hostAdded"),
          );
        },
        onError: (error) => {
          toast.error(translate("admin.selfSignedCA.addFailed"), {
            description: extractErrorMessage(
              error,
              translate("admin.selfSignedCA.unknownError"),
            ),
          });
        },
      },
    );
  };

  const confirmRemoveHost = async (value: string) => {
    removingHost.value = value;
    await runRemoveHostAction(() => ConfigAPI.removeCAHost(value), {
      onSuccess: (nextHosts) => {
        hosts.value = nextHosts;
        toast.success(translate("admin.selfSignedCA.hostRemoved"));
      },
      onError: (error) => {
        toast.error(translate("admin.selfSignedCA.removeFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.selfSignedCA.unknownError"),
          ),
        });
      },
      onFinally: () => {
        removingHost.value = null;
      },
    });
  };

  const generateRootCA = async () => {
    await runBusyAction(() => ConfigAPI.initCA(), {
      onSuccess: async () => {
        await refreshCAStatus();
        toast.success(translate("admin.selfSignedCA.rootGenerated"));
      },
      onError: (error) => {
        toast.error(translate("admin.selfSignedCA.generateFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.selfSignedCA.unknownError"),
          ),
        });
      },
    });
  };

  const confirmFinalClear = async () => {
    await runClearRootCA(() => ConfigAPI.clearCA(), {
      onSuccess: () => {
        caInfo.value = null;
        hasRootCA.value = false;
        toast.success(translate("admin.selfSignedCA.rootCleared"));
        showSecondConfirm.value = false;
      },
    });
  };

  const confirmFinalRegen = async () => {
    await runRegenerateRootCA(() => ConfigAPI.initCA(), {
      onSuccess: async () => {
        await refreshCAStatus();
        toast.success(translate("admin.selfSignedCA.rootRegenerated"));
        showRegenSecondConfirm.value = false;
      },
    });
  };

  const issueAndInstall = async () => {
    if (!hasRootCA.value || !hosts.value.length) return;
    await runBusyAction(() => ConfigAPI.issueAndInstall(), {
      onSuccess: ({ success, message }) => {
        if (success) {
          toast.success(
            translate("admin.selfSignedCA.certificateIssuedInstalled"),
          );
          return;
        }
        toast.error(translate("admin.selfSignedCA.issueFailed"), {
          description:
            message || translate("admin.selfSignedCA.unknownError"),
        });
      },
      onError: (error) => {
        toast.error(translate("admin.selfSignedCA.issueFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.selfSignedCA.unknownError"),
          ),
        });
      },
    });
  };

  const formatDate = (dateString: string) => {
    const date = new Date(dateString);
    if (Number.isNaN(date.getTime())) return dateString;
    return date.toLocaleDateString(locale.value, {
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const downloadCA = async () => {
    await runDownloadFile(async () => {
      const blob = await ConfigAPI.downloadCACert();
      downloadBlob(blob, "KCI-LNK-Root-CA.pem");
    });
  };
  const downloadServer = async () => {
    await runDownloadFile(async () => {
      const blob = await ConfigAPI.downloadServerCert();
      downloadBlob(blob, "server-cert.zip");
    });
  };

  onMounted(() => {
    void refreshCAStatus();
  });

  return {
    addHost,
    caInfo,
    confirmFinalClear,
    confirmFinalRegen,
    confirmFirst: () => {
      showFirstConfirm.value = false;
      showSecondConfirm.value = true;
    },
    confirmRegenFirst: () => {
      showRegenFirstConfirm.value = false;
      showRegenSecondConfirm.value = true;
    },
    confirmRemoveHost,
    downloadCA,
    downloadServer,
    formatDate,
    generateRootCA,
    hasRootCA,
    hosts,
    isBusy,
    isClearing,
    isDownloading,
    isIP,
    isInitializing,
    isRegenerating,
    isRemoving,
    issueAndInstall,
    newHost,
    openFirstConfirm: () => {
      showFirstConfirm.value = true;
    },
    openRegenFirstConfirm: () => {
      showRegenFirstConfirm.value = true;
    },
    pendingHosts,
    removingHost,
    showFirstConfirm,
    showInitializingSkeleton,
    showRegenFirstConfirm,
    showRegenSecondConfirm,
    showSecondConfirm,
  };
};
