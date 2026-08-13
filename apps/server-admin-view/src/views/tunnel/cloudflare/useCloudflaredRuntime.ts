import { computed, ref, watch } from "vue";
import { useRouter } from "vue-router";
import {
  CloudflaredAPI,
  type CloudflaredConfig,
  type CloudflaredProtocol,
  type TunnelSupervisorStatus,
} from "@/lib/api/tunnel";
import { ConfigAPI } from "@/lib/api/config";
import { SystemAPI } from "@/lib/api/system";
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
import type {
  CloudflaredProtocolOption,
  CloudflareTranslate,
} from "./cloudflareTunnelTypes";

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

export const useCloudflaredRuntime = ({
  onConfigLoaded,
  t,
}: {
  onConfigLoaded: (config: CloudflaredConfig) => void;
  t: CloudflareTranslate;
}) => {
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
  const configLoaded = ref(false);
  const hasCloudflaredLogBaseline = ref(false);
  const token = ref("");
  const tunnelTokenConfigured = ref(false);
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
        protocol.value = config.protocol || "auto";
        onConfigLoaded(config);
      },
      { onFinally: () => (configLoaded.value = true) },
    );
  };

  const selectAsDefaultTunnel = async () => {
    await ConfigAPI.updateDefaultTunnel("cloudflared");
    if (configStore.config) configStore.config.default_tunnel = "cloudflared";
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
      toast.success(
        t(
          shouldRestart
            ? "admin.cloudflareTunnel.restartSuccess"
            : "admin.cloudflareTunnel.saveSuccess",
        ),
      );
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
  const loadEnvironmentConfig = () =>
    configStore.config ? Promise.resolve() : configStore.loadConfig();

  return {
    authServiceHost,
    canStart,
    canStop,
    cloudflaredLogAnalysis,
    cloudflaredLogAnalysisMessage,
    cloudflaredOriginServiceUrl,
    cloudflaredProtocolDescription,
    cloudflaredProtocolLabel,
    cloudflaredProtocolOptions,
    configLoaded,
    gotoResources,
    hasSubdomainRoot,
    isClearingLogs,
    isReverseProxySubdomainMode,
    isSaving,
    isStarting,
    isStopping,
    loadAccessEntryPort,
    loadConfig,
    loadEnvironmentConfig,
    loadStatus,
    logs,
    onClearLogsClick,
    pid,
    protocol,
    publicWildcardHostname,
    running,
    saveConfig,
    showInitDialog,
    showToken,
    startCloudflared,
    startPolling: cloudflaredPolling.start,
    stopCloudflared,
    stopPolling: cloudflaredPolling.stop,
    supervisor,
    token,
    tunnelTokenConfigured,
  };
};
