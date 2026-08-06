import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  CloudflaredAPI,
  ConfigAPI,
  SystemAPI,
  type CloudflareManagedState,
  type CloudflareOptimizationScan,
  type CloudflareReconcilePlan,
  type CloudflaredProtocol,
  type TunnelSupervisorStatus,
} from "@/lib/api";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  DEFAULT_LOG_WINDOW_SIZE,
  mergePollingLogWindow,
} from "@admin-shared/utils/log-window";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { useTargetPolling } from "@/composables/useTargetPolling";
import { useConfigStore } from "@/store/config";
import {
  analyzeCloudflaredLogs,
  type CloudflaredLogAnalysis,
} from "./cloudflaredLogAnalysis";

type CloudflaredProtocolOption = {
  value: CloudflaredProtocol;
  label: string;
  description: string;
};

const stoppedSupervisor = (): TunnelSupervisorStatus => ({
  state: "stopped",
  desiredRunning: false,
  running: false,
  attached: false,
  pid: null,
  restartCount: 0,
  consecutiveFailures: 0,
  nextRestartAt: null,
  startedAt: null,
  stoppedAt: null,
  lastFailure: null,
  lastMessage: null,
});

export const useCloudflareTunnelController = () => {
  const { locale, t } = useI18n();
  const router = useRouter();
  const configStore = useConfigStore();
  const isInit = ref(false);
  const running = ref(false);
  const pid = ref<number | null>(null);
  const supervisor = ref<TunnelSupervisorStatus>(stoppedSupervisor());
  const logs = ref<string[]>([]);
  const cloudflaredLogAnalysis = ref<CloudflaredLogAnalysis | null>(null);
  const showInitDialog = ref(false);
  const showToken = ref(false);
  const showApiToken = ref(false);
  const configLoaded = ref(false);
  const hasCloudflaredLogBaseline = ref(false);
  const token = ref("");
  const tunnelTokenConfigured = ref(false);
  const apiToken = ref("");
  const apiTokenConfigured = ref(false);
  const managedState = ref<CloudflareManagedState | null>(null);
  const tunnelMode = ref<"dedicated" | "existing">("dedicated");
  const selectedTunnelId = ref("");
  const optimizationEnabled = ref(false);
  const reconcilePlan = ref<CloudflareReconcilePlan | null>(null);
  const takeoverResourceIds = ref<string[]>([]);
  const optimizationScan = ref<CloudflareOptimizationScan | null>(null);
  const selectedCandidateIp = ref("");
  const optimizationOfficialRanges = ref(true);
  const optimizationBuiltinIds = ref<string[]>([]);
  const optimizationCustomHostnames = ref("");
  const isConnectingCloudflare = ref(false);
  const isLoadingManagedState = ref(false);
  const isPreviewingReconcile = ref(false);
  const isApplyingReconcile = ref(false);
  const deleteDedicatedTunnel = ref(false);
  const isScanningOptimization = ref(false);
  const isApplyingOptimization = ref(false);
  const isFallingBackOptimization = ref(false);
  const isSavingOptimizationSources = ref(false);
  let optimizationSourcesLoaded = false;
  let scanPollTimer: number | undefined;
  let managedStatePollTimer: number | undefined;
  const protocol = ref<CloudflaredProtocol>("auto");
  const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort({
    onError: (error) => {
      console.warn("load cloudflared access entry port failed:", error);
    },
  });

  const cloudflaredProtocolOptions = computed<CloudflaredProtocolOption[]>(
    () => [
      {
        value: "auto",
        label: t("admin.cloudflareTunnel.protocol.auto"),
        description: t("admin.cloudflareTunnel.protocol.autoDescription"),
      },
      {
        value: "http2",
        label: "HTTP2",
        description: t("admin.cloudflareTunnel.protocol.http2Description"),
      },
      {
        value: "quic",
        label: "QUIC",
        description: t("admin.cloudflareTunnel.protocol.quicDescription"),
      },
    ],
  );
  const defaultCloudflaredProtocolOption = computed<CloudflaredProtocolOption>(
    () =>
      cloudflaredProtocolOptions.value[0] ?? {
        value: "auto",
        label: t("admin.cloudflareTunnel.protocol.auto"),
        description: t("admin.cloudflareTunnel.protocol.autoDescription"),
      },
  );

  const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.cloudflareTunnel.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.saveFailed"),
        ),
      });
    },
  });
  const { isPending: isStarting, run: runStartCloudflared } = useAsyncAction();
  const { isPending: isStopping, run: runStopCloudflared } = useAsyncAction();
  const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.cloudflareTunnel.clearLogsFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.clearLogsFailed"),
        ),
      });
    },
  });
  const { run: runLoadStatus } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.cloudflareTunnel.loadStatusFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.loadStatusFailed"),
        ),
      });
    },
  });
  const { run: runLoadConfig } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.cloudflareTunnel.loadConfigFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.loadConfigFailed"),
        ),
      });
    },
  });

  watch(token, (newValue) => {
    if (!newValue) return;
    const rawTokenMatch = newValue.match(/(eyJ[A-Za-z0-9-_]+)/);
    if (rawTokenMatch?.[1] && newValue !== rawTokenMatch[1]) {
      token.value = rawTokenMatch[1];
      toast.success(t("admin.cloudflareTunnel.tokenExtracted"));
    }
  });

  const canStart = computed(
    () =>
      isInit.value &&
      !supervisor.value.desiredRunning &&
      !supervisor.value.running &&
      (tunnelTokenConfigured.value || token.value.trim()),
  );
  const canStop = computed(
    () => supervisor.value.desiredRunning || supervisor.value.running,
  );
  const isReverseProxySubdomainMode = computed(
    () =>
      configStore.config?.run_type === 1 &&
      configStore.config?.reverse_proxy_submode === "subdomain",
  );
  const rootDomain = computed(
    () =>
      configStore.config?.subdomain_mode?.root_domain?.trim().toLowerCase() ||
      "",
  );
  const publicWildcardHostname = computed(() =>
    rootDomain.value ? `*.${rootDomain.value}` : "*.example.com",
  );
  const authServiceHost = computed(() => {
    const authMapping = configStore.config?.host_mappings?.find(
      (mapping) => mapping.service_role === "auth",
    );
    return (
      authMapping?.host?.trim() ||
      configStore.config?.subdomain_mode?.auth_host?.trim() ||
      ""
    );
  });
  const displayAccessEntryPort = computed(
    () => accessEntryPort.value.trim() || "7999",
  );
  const cloudflaredOriginServiceUrl = computed(
    () => `http://127.0.0.1:${displayAccessEntryPort.value}`,
  );
  const hasSubdomainRoot = computed(() => Boolean(rootDomain.value));
  const cloudflaredProtocolOption = computed(
    () =>
      cloudflaredProtocolOptions.value.find(
        (option) => option.value === protocol.value,
      ) ?? defaultCloudflaredProtocolOption.value,
  );
  const cloudflaredProtocolLabel = computed(
    () => cloudflaredProtocolOption.value.label,
  );
  const cloudflaredProtocolDescription = computed(
    () => cloudflaredProtocolOption.value.description,
  );
  const cloudflaredLogAnalysisMessage = computed(() => {
    const analysis = cloudflaredLogAnalysis.value;
    if (!analysis) return "";
    const originTarget = analysis.originHost
      ? t("admin.cloudflareTunnel.analysisOriginHost", {
          host: analysis.originHost,
        })
      : t("admin.cloudflareTunnel.analysisOriginGeneric");
    return t("admin.cloudflareTunnel.analysisMessage", {
      origin: originTarget,
      certificates: analysis.certificateHosts.join(", "),
      requested: analysis.requestedHost,
    });
  });

  const optimization = computed(() => managedState.value?.optimization ?? null);
  const optimizationApplied = computed(
    () => optimization.value?.enabled === true,
  );
  const optimizationScanReady = computed(
    () => optimization.value?.scanReady === true,
  );
  const optimizationReadinessErrorCode = computed(
    () => optimization.value?.scanReadinessErrorCode ?? null,
  );
  const reconcileHasUnconfirmedConflicts = computed(() => {
    const plan = reconcilePlan.value;
    if (!plan) return false;
    return plan.conflicts.some(
      (conflict) =>
        !conflict.takeoverAllowed ||
        !takeoverResourceIds.value.includes(conflict.id),
    );
  });

  const loadStatus = async () => {
    await runLoadStatus(async () => {
      const status = await CloudflaredAPI.getStatus();
      isInit.value = status.initialized;
      running.value = status.running;
      pid.value = status.pid;
      supervisor.value = status.supervisor;
      if (!isInit.value) {
        const systemStatus = await SystemAPI.getCloudflaredStatus();
        if (!systemStatus?.data?.downloaded) showInitDialog.value = true;
      }
    });
  };
  const loadConfig = async () => {
    await runLoadConfig(
      async () => {
        const config = await CloudflaredAPI.getConfig();
        token.value = "";
        tunnelTokenConfigured.value = config.tunnelTokenConfigured;
        apiTokenConfigured.value = config.apiTokenConfigured;
        protocol.value = config.protocol || "auto";
        optimizationEnabled.value = config.optimizationEnabled;
        if (config.tunnel?.ownership === "adopted") {
          tunnelMode.value = "existing";
          selectedTunnelId.value = config.tunnel.id;
        }
      },
      { onFinally: () => (configLoaded.value = true) },
    );
  };
  const loadManagedState = async (options?: { silent?: boolean }) => {
    if (isLoadingManagedState.value) return;
    isLoadingManagedState.value = true;
    try {
      const next = await CloudflaredAPI.getCloudflareState();
      managedState.value = next;
      apiTokenConfigured.value = next.apiTokenConfigured;
      tunnelTokenConfigured.value = next.tunnelTokenConfigured;
      optimizationEnabled.value = next.optimization.enabled;
      if (!optimizationSourcesLoaded) {
        const sources = next.optimization.candidateSources;
        optimizationOfficialRanges.value = sources.officialRanges;
        optimizationBuiltinIds.value = sources.builtins
          .filter((source) => source.enabled)
          .map((source) => source.id);
        optimizationCustomHostnames.value = sources.customHostnames.join("\n");
        optimizationSourcesLoaded = true;
      }
      const managedTunnel = next.managed.tunnel;
      if (managedTunnel?.ownership === "adopted") {
        tunnelMode.value = "existing";
        selectedTunnelId.value = managedTunnel.id;
      } else if (managedTunnel?.ownership === "dedicated") {
        tunnelMode.value = "dedicated";
      }
      const latestScan = next.optimization.scans[0];
      if (latestScan && !optimizationScan.value) {
        optimizationScan.value = latestScan;
      }
    } catch (error) {
      if (!options?.silent) {
        toast.error(t("admin.cloudflareTunnel.managed.loadFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.cloudflareTunnel.managed.loadFailed"),
          ),
        });
      }
    } finally {
      isLoadingManagedState.value = false;
    }
  };

  const connectCloudflare = async () => {
    const value = apiToken.value.trim();
    if (!value) return;
    isConnectingCloudflare.value = true;
    try {
      managedState.value = await CloudflaredAPI.saveCloudflareCredential(value);
      apiToken.value = "";
      apiTokenConfigured.value = true;
      toast.success(t("admin.cloudflareTunnel.managed.connected"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.connectFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.connectFailed"),
        ),
      });
    } finally {
      isConnectingCloudflare.value = false;
    }
  };

  const disconnectCloudflare = async () => {
    isConnectingCloudflare.value = true;
    try {
      await CloudflaredAPI.deleteCloudflareCredential();
      apiTokenConfigured.value = false;
      apiToken.value = "";
      if (managedState.value) managedState.value.apiTokenConfigured = false;
      toast.success(t("admin.cloudflareTunnel.managed.disconnected"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.disconnectFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.disconnectFailed"),
        ),
      });
    } finally {
      isConnectingCloudflare.value = false;
    }
  };

  const previewReconcile = async () => {
    isPreviewingReconcile.value = true;
    reconcilePlan.value = null;
    takeoverResourceIds.value = [];
    try {
      reconcilePlan.value = await CloudflaredAPI.previewReconcile({
        tunnelMode: tunnelMode.value,
        tunnelId:
          tunnelMode.value === "existing"
            ? selectedTunnelId.value || undefined
            : undefined,
        optimizationEnabled: optimizationEnabled.value,
      });
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.previewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.previewFailed"),
        ),
      });
    } finally {
      isPreviewingReconcile.value = false;
    }
  };

  const previewCleanup = async () => {
    isPreviewingReconcile.value = true;
    reconcilePlan.value = null;
    takeoverResourceIds.value = [];
    try {
      reconcilePlan.value = await CloudflaredAPI.previewReconcile({
        action: "cleanup",
        tunnelMode: tunnelMode.value,
        tunnelId: selectedTunnelId.value || undefined,
        optimizationEnabled: optimizationEnabled.value,
        deleteDedicatedTunnel: deleteDedicatedTunnel.value,
      });
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.previewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.previewFailed"),
        ),
      });
    } finally {
      isPreviewingReconcile.value = false;
    }
  };

  const applyReconcile = async () => {
    if (!reconcilePlan.value || reconcileHasUnconfirmedConflicts.value) return;
    isApplyingReconcile.value = true;
    try {
      managedState.value = await CloudflaredAPI.applyReconcile({
        planId: reconcilePlan.value.planId,
        takeoverResourceIds: takeoverResourceIds.value,
      });
      tunnelTokenConfigured.value = managedState.value.tunnelTokenConfigured;
      reconcilePlan.value = null;
      takeoverResourceIds.value = [];
      await loadConfig();
      toast.success(t("admin.cloudflareTunnel.managed.applied"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.managed.applyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.managed.applyFailed"),
        ),
      });
    } finally {
      isApplyingReconcile.value = false;
    }
  };

  const pollOptimizationScan = async (id: string) => {
    if (scanPollTimer) window.clearTimeout(scanPollTimer);
    try {
      const scan = await CloudflaredAPI.getOptimizationScan(id);
      optimizationScan.value = scan;
      isScanningOptimization.value = ["queued", "running"].includes(
        scan.status,
      );
      if (scan.status === "completed") {
        selectedCandidateIp.value = scan.recommendedIp || "";
        await loadManagedState({ silent: true });
        return;
      }
      if (["failed", "cancelled"].includes(scan.status)) return;
      scanPollTimer = window.setTimeout(
        () => void pollOptimizationScan(id),
        1500,
      );
    } catch (error) {
      isScanningOptimization.value = false;
      toast.error(t("admin.cloudflareTunnel.optimization.scanFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.scanFailed"),
        ),
      });
    }
  };

  const startOptimizationScan = async () => {
    if (!optimizationApplied.value) {
      toast.warning(
        t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle"),
        {
          description: t(
            "admin.cloudflareTunnel.optimization.reconcileRequiredDescription",
          ),
        },
      );
      return;
    }
    if (!optimizationScanReady.value) {
      const resourceConflict =
        optimizationReadinessErrorCode.value === "cloudflare-resource-conflict";
      const validationPending =
        optimizationReadinessErrorCode.value ===
        "cloudflare-saas-validation-pending";
      toast.warning(
        t(
          resourceConflict
            ? "admin.cloudflareTunnel.optimization.resourceConflictTitle"
            : validationPending
              ? "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingTitle"
              : "admin.cloudflareTunnel.optimization.notReadyTitle",
        ),
        {
          description: t(
            resourceConflict
              ? "admin.cloudflareTunnel.optimization.resourceConflictDescription"
              : validationPending
                ? "admin.cloudflareTunnel.optimization.cloudflareSaasValidationPendingDescription"
                : "admin.cloudflareTunnel.optimization.notReadyDescription",
          ),
        },
      );
      return;
    }
    isScanningOptimization.value = true;
    try {
      const scan = await CloudflaredAPI.startOptimizationScan();
      optimizationScan.value = scan;
      await pollOptimizationScan(scan.id);
    } catch (error) {
      isScanningOptimization.value = false;
      toast.error(t("admin.cloudflareTunnel.optimization.scanFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.scanFailed"),
        ),
      });
    }
  };

  const toggleOptimizationBuiltin = (id: string, enabled: boolean) => {
    const current = new Set(optimizationBuiltinIds.value);
    if (enabled) current.add(id);
    else current.delete(id);
    optimizationBuiltinIds.value = [...current];
  };

  const saveOptimizationSources = async () => {
    isSavingOptimizationSources.value = true;
    try {
      const sources = await CloudflaredAPI.saveOptimizationSourceSettings({
        officialRanges: optimizationOfficialRanges.value,
        builtinIds: optimizationBuiltinIds.value,
        customHostnames: optimizationCustomHostnames.value
          .split(/[\n,]+/u)
          .map((value) => value.trim())
          .filter(Boolean),
      });
      optimizationOfficialRanges.value = sources.officialRanges;
      optimizationBuiltinIds.value = sources.builtins
        .filter((source) => source.enabled)
        .map((source) => source.id);
      optimizationCustomHostnames.value = sources.customHostnames.join("\n");
      if (managedState.value) {
        managedState.value.optimization.candidateSources = sources;
      }
      toast.success(t("admin.cloudflareTunnel.optimization.sources.saved"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.sources.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.sources.saveFailed"),
        ),
      });
    } finally {
      isSavingOptimizationSources.value = false;
    }
  };

  const cancelOptimizationScan = async () => {
    const scan = optimizationScan.value;
    if (!scan) return;
    try {
      await CloudflaredAPI.cancelOptimizationScan(scan.id);
      if (scanPollTimer) window.clearTimeout(scanPollTimer);
      isScanningOptimization.value = false;
      optimizationScan.value = {
        ...scan,
        status: "cancelled",
        phase: "cancelled",
      };
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.cancelFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.cancelFailed"),
        ),
      });
    }
  };

  const applyOptimization = async () => {
    const scan = optimizationScan.value;
    if (!scan || scan.status !== "completed" || !optimizationApplied.value) {
      if (!optimizationApplied.value) {
        toast.warning(
          t("admin.cloudflareTunnel.optimization.reconcileRequiredTitle"),
          {
            description: t(
              "admin.cloudflareTunnel.optimization.reconcileRequiredDescription",
            ),
          },
        );
      }
      return;
    }
    isApplyingOptimization.value = true;
    try {
      await CloudflaredAPI.applyOptimization({
        scanId: scan.id,
        candidateIp: selectedCandidateIp.value || undefined,
      });
      await loadManagedState({ silent: true });
      toast.success(t("admin.cloudflareTunnel.optimization.applied"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.applyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.applyFailed"),
        ),
      });
    } finally {
      isApplyingOptimization.value = false;
    }
  };

  const fallbackOptimization = async () => {
    isFallingBackOptimization.value = true;
    try {
      await CloudflaredAPI.fallbackOptimization();
      await loadManagedState({ silent: true });
      toast.success(t("admin.cloudflareTunnel.optimization.fallbackApplied"));
    } catch (error) {
      toast.error(t("admin.cloudflareTunnel.optimization.fallbackFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.cloudflareTunnel.optimization.fallbackFailed"),
        ),
      });
    } finally {
      isFallingBackOptimization.value = false;
    }
  };
  const selectAsDefaultTunnel = async () => {
    await ConfigAPI.updateDefaultTunnel("cloudflared");
    if (configStore.config) {
      configStore.config.default_tunnel = "cloudflared";
    }
  };
  const startCloudflared = async (options?: { silent?: boolean }) => {
    await runStartCloudflared(() => CloudflaredAPI.start(), {
      onSuccess: async (result) => {
        pid.value = result.pid;
        running.value = true;
        supervisor.value = {
          ...supervisor.value,
          state: "running",
          desiredRunning: true,
          running: true,
          pid: result.pid,
          nextRestartAt: null,
        };
        await selectAsDefaultTunnel();
        if (!options?.silent) {
          toast.success(t("admin.cloudflareTunnel.startSuccess"));
        }
      },
      onError: (error) => {
        if (options?.silent) return;
        toast.error(t("admin.cloudflareTunnel.startFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.cloudflareTunnel.startFailed"),
          ),
        });
      },
    });
  };
  const stopCloudflared = async (options?: { silent?: boolean }) => {
    await runStopCloudflared(() => CloudflaredAPI.stop(), {
      onSuccess: () => {
        running.value = false;
        pid.value = null;
        supervisor.value = stoppedSupervisor();
        if (!options?.silent) {
          toast.success(t("admin.cloudflareTunnel.stopSuccess"));
        }
      },
      onError: (error) => {
        if (options?.silent) return;
        toast.error(t("admin.cloudflareTunnel.stopFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.cloudflareTunnel.stopFailed"),
          ),
        });
      },
    });
  };
  const saveConfig = async () => {
    await runSaveConfig(async () => {
      const shouldRestart = supervisor.value.desiredRunning;
      await CloudflaredAPI.saveConfig({
        protocol: protocol.value,
        ...(token.value.trim() ? { token: token.value.trim() } : {}),
      });
      if (token.value.trim()) {
        tunnelTokenConfigured.value = true;
        token.value = "";
      }
      await loadStatus();
      if (shouldRestart) {
        toast.success(t("admin.cloudflareTunnel.restartSuccess"));
        return;
      }
      toast.success(t("admin.cloudflareTunnel.saveSuccess"));
    });
  };

  const cloudflaredPolling = useTargetPolling({
    target: "cloudflared",
    intervalMs: 2000,
    onData: (payload) => {
      logs.value = mergePollingLogWindow(logs.value, payload.logs, {
        reset: payload.reset,
        max: DEFAULT_LOG_WINDOW_SIZE,
      });
      running.value = payload.status.running;
      pid.value = payload.status.pid;
      supervisor.value = payload.status.supervisor;
      if (!hasCloudflaredLogBaseline.value) {
        hasCloudflaredLogBaseline.value = true;
        return;
      }
      const analysis = analyzeCloudflaredLogs(payload.logs);
      if (analysis) cloudflaredLogAnalysis.value = analysis;
    },
  });

  const onClearLogsClick = async () => {
    await runClearLogs(() => CloudflaredAPI.clearLogs(), {
      onSuccess: () => {
        logs.value = [];
        cloudflaredLogAnalysis.value = null;
        cloudflaredPolling.resetCursor();
        void cloudflaredPolling.refresh();
        toast.success(t("admin.cloudflareTunnel.logsCleared"));
      },
    });
  };
  const gotoResources = () => {
    showInitDialog.value = false;
    void router.push({ path: "/system", query: { tab: "cloudflared" } });
  };

  onMounted(async () => {
    await Promise.all([
      loadStatus(),
      loadConfig(),
      loadAccessEntryPort(),
      configStore.config ? Promise.resolve() : configStore.loadConfig(),
      loadManagedState({ silent: true }),
    ]);
    cloudflaredPolling.start();
    managedStatePollTimer = window.setInterval(() => {
      if (apiTokenConfigured.value) {
        void loadManagedState({ silent: true });
      }
    }, 60_000);
  });
  onUnmounted(() => {
    cloudflaredPolling.stop();
    if (scanPollTimer) window.clearTimeout(scanPollTimer);
    if (managedStatePollTimer) window.clearInterval(managedStatePollTimer);
  });

  return {
    authServiceHost,
    apiToken,
    apiTokenConfigured,
    applyOptimization,
    applyReconcile,
    cancelOptimizationScan,
    canStart,
    canStop,
    cloudflaredLogAnalysis,
    cloudflaredLogAnalysisMessage,
    cloudflaredOriginServiceUrl,
    cloudflaredProtocolDescription,
    cloudflaredProtocolLabel,
    cloudflaredProtocolOptions,
    configLoaded,
    connectCloudflare,
    disconnectCloudflare,
    deleteDedicatedTunnel,
    fallbackOptimization,
    gotoResources,
    hasSubdomainRoot,
    isClearingLogs,
    isApplyingOptimization,
    isApplyingReconcile,
    isConnectingCloudflare,
    isFallingBackOptimization,
    isSavingOptimizationSources,
    isLoadingManagedState,
    isPreviewingReconcile,
    isReverseProxySubdomainMode,
    isSaving,
    isScanningOptimization,
    isStarting,
    isStopping,
    logs,
    locale,
    managedState,
    onClearLogsClick,
    pid,
    optimization,
    optimizationApplied,
    optimizationEnabled,
    optimizationBuiltinIds,
    optimizationCustomHostnames,
    optimizationOfficialRanges,
    optimizationReadinessErrorCode,
    optimizationScan,
    optimizationScanReady,
    protocol,
    publicWildcardHostname,
    reconcileHasUnconfirmedConflicts,
    reconcilePlan,
    running,
    saveConfig,
    saveOptimizationSources,
    showInitDialog,
    showApiToken,
    showToken,
    selectedCandidateIp,
    selectedTunnelId,
    startOptimizationScan,
    startCloudflared,
    stopCloudflared,
    supervisor,
    takeoverResourceIds,
    toggleOptimizationBuiltin,
    t,
    token,
    tunnelMode,
    tunnelTokenConfigured,
    previewReconcile,
    previewCleanup,
  };
};

export type CloudflareTunnelController = ReturnType<
  typeof useCloudflareTunnelController
>;
