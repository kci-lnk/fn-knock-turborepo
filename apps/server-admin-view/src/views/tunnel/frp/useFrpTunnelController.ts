import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { ConfigAPI } from "@/lib/api/config";
import {
  FrpcAPI,
  type FrpcInstanceStatus,
  type FrpcInstanceSummary,
  type FrpcInstancesOverview,
} from "@/lib/api/tunnel";
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
import { useTargetPolling } from "@/composables/useTargetPolling";
import { useConfigStore } from "@/store/config";
import {
  replaceFrpcOverviewItem,
  summarizeFrpcContent,
} from "./frpcInstanceModel";

type FrpcEditorExpose = {
  getContent: () => string;
  resetFromRaw: (raw: string) => void;
};

const START_ERROR_WATCH_MS = 30_000;
const CONNECTION_REFUSED_REGEX = /\bconnection refused\b/i;

export const useFrpTunnelController = () => {
  const router = useRouter();
  const configStore = useConfigStore();
  const { t } = useI18n();
  const overview = ref<FrpcInstancesOverview | null>(null);
  const primaryConfig = ref("");
  const primaryLogs = ref<string[]>([]);
  const showInitDialog = ref(false);
  const configLoaded = ref(false);
  const primaryEditorRef = ref<FrpcEditorExpose | null>(null);
  const setPrimaryEditorRef = (editor: unknown) => {
    primaryEditorRef.value = editor as FrpcEditorExpose | null;
  };
  const startingInstanceId = ref<string | null>(null);
  const stoppingInstanceId = ref<string | null>(null);
  const deletingInstanceId = ref<string | null>(null);
  const startErrorTrace = ref<{
    pid: number;
    markerSeen: boolean;
    expireAt: number;
  } | null>(null);

  const defaults = computed(
    () => overview.value?.defaults ?? { local_port: "7999" },
  );
  const primaryInstance = computed(
    () =>
      overview.value?.items.find(
        (item) => item.id === overview.value?.primaryInstanceId,
      ) ?? null,
  );
  const extraInstances = computed(
    () => overview.value?.items.filter((item) => !item.isPrimary) ?? [],
  );
  const isInit = computed(() => overview.value?.initialized ?? false);
  const running = computed(() => primaryInstance.value?.running ?? false);
  const pid = computed(() => primaryInstance.value?.pid ?? null);
  const canStart = computed(
    () =>
      isInit.value &&
      !primaryInstance.value?.desiredRunning &&
      !primaryInstance.value?.running,
  );
  const canStop = computed(
    () =>
      (primaryInstance.value?.desiredRunning ?? false) ||
      (primaryInstance.value?.running ?? false),
  );
  const primarySummary = computed(
    () =>
      primaryInstance.value?.summary ??
      summarizeFrpcContent(primaryConfig.value, defaults.value.local_port),
  );

  const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.frpTunnel.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.saveFailed"),
        ),
      });
    },
  });
  const { isPending: isStarting, run: runStartFrpc } = useAsyncAction();
  const { isPending: isStopping, run: runStopFrpc } = useAsyncAction();
  const { isPending: isClearingLogs, run: runClearLogs } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.frpTunnel.clearLogsFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.clearLogsFailed"),
        ),
      });
    },
  });
  const { run: runLoadStatus } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.frpTunnel.loadStatusFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.loadStatusFailed"),
        ),
      });
    },
  });
  const { run: runLoadConfig } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.frpTunnel.loadConfigFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.loadConfigFailed"),
        ),
      });
    },
  });

  const formatSummary = (summary: FrpcInstanceSummary) => {
    const server = summary.serverAddr
      ? `${summary.serverAddr}:${summary.serverPort || "7000"}`
      : t("admin.frpTunnel.notConfigured");
    return t("admin.frpTunnel.summary", {
      server,
      local: summary.localPort || defaults.value.local_port,
      remote: summary.remotePort || "0",
    });
  };

  const getInstanceDisplayName = (
    instance: FrpcInstanceStatus | null | undefined,
  ) => {
    if (!instance) return t("admin.frpTunnel.instance");
    const name = instance.name.trim();
    if (name) return name;
    if (instance.summary.serverAddr) {
      return `${instance.summary.serverAddr}:${instance.summary.serverPort || "7000"}`;
    }
    return instance.isPrimary
      ? t("admin.frpTunnel.primaryFrp")
      : t("admin.frpTunnel.instance");
  };

  const updateOverviewItem = (item: FrpcInstanceStatus) => {
    if (overview.value) {
      overview.value = replaceFrpcOverviewItem(overview.value, item);
    }
  };

  const gotoInstanceCreate = () => {
    void router.push({ path: "/tunnel/frp/instances/new" });
  };

  const gotoInstanceDetail = (
    instance: FrpcInstanceStatus,
    section?: "config" | "logs",
  ) => {
    void router.push({
      path: `/tunnel/frp/instances/${encodeURIComponent(instance.id)}`,
      query: section ? { section } : undefined,
    });
  };

  const loadStatus = async () => {
    await runLoadStatus(async () => {
      const data = await FrpcAPI.getInstances();
      overview.value = data;
      if (!data.initialized) {
        const sys = await SystemAPI.getFrpStatus();
        if (!sys?.data?.downloaded) showInitDialog.value = true;
      }
    });
  };

  const loadConfig = async () => {
    await runLoadConfig(
      async () => {
        const raw = await FrpcAPI.getConfig();
        primaryConfig.value = raw;
        primaryEditorRef.value?.resetFromRaw(raw);
      },
      { onFinally: () => (configLoaded.value = true) },
    );
  };

  const markStarted = (pid: number) => {
    startErrorTrace.value = {
      pid,
      markerSeen: false,
      expireAt: Date.now() + START_ERROR_WATCH_MS,
    };
  };

  const saveConfig = async () => {
    await runSaveConfig(async () => {
      const content =
        primaryEditorRef.value?.getContent() ?? primaryConfig.value;
      const shouldRestart = primaryInstance.value?.desiredRunning ?? false;
      await FrpcAPI.saveConfig(content);
      primaryConfig.value = content;
      if (shouldRestart) {
        toast.success(t("admin.frpTunnel.restartSuccess"));
      } else {
        toast.success(t("admin.frpTunnel.saveSuccess"));
      }
      await loadStatus();
    });
  };

  const selectFrpAsDefaultTunnel = async () => {
    await ConfigAPI.updateDefaultTunnel("frp");
    if (configStore.config) configStore.config.default_tunnel = "frp";
  };

  const startFrpc = async (options?: { silent?: boolean }) => {
    await runStartFrpc(() => FrpcAPI.start(), {
      onSuccess: async (result) => {
        markStarted(result.pid);
        await selectFrpAsDefaultTunnel();
        await loadStatus();
        if (!options?.silent) toast.success(t("admin.frpTunnel.startSuccess"));
      },
      onError: (error) => {
        if (options?.silent) return;
        const message = extractErrorMessage(
          error,
          t("admin.frpTunnel.startFailed"),
        );
        if (CONNECTION_REFUSED_REGEX.test(message)) {
          toast.error(t("admin.frpTunnel.startFailed"), {
            description: t("admin.frpTunnel.connectionRefused"),
          });
          return;
        }
        toast.error(t("admin.frpTunnel.startFailed"), { description: message });
      },
    });
  };

  const stopFrpc = async (options?: { silent?: boolean }) => {
    await runStopFrpc(() => FrpcAPI.stop(), {
      onSuccess: async () => {
        await loadStatus();
        if (!options?.silent) toast.success(t("admin.frpTunnel.stopSuccess"));
      },
      onError: (error) => {
        if (options?.silent) return;
        toast.error(t("admin.frpTunnel.stopFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.frpTunnel.stopFailed"),
          ),
        });
      },
    });
  };

  const frpcPolling = useTargetPolling({
    target: "frpc",
    intervalMs: 2000,
    onData: (payload) => {
      primaryLogs.value = mergePollingLogWindow(
        primaryLogs.value,
        payload.logs,
        { reset: payload.reset, max: DEFAULT_LOG_WINDOW_SIZE },
      );
      if (payload.status.instances) {
        overview.value = payload.status.instances;
      } else {
        updateOverviewItem(payload.status);
      }
      handleStartFailureLogs(payload.logs);
    },
  });

  const onClearLogsClick = async () => {
    await runClearLogs(() => FrpcAPI.clearLogs(), {
      onSuccess: () => {
        primaryLogs.value = [];
        frpcPolling.resetCursor();
        void frpcPolling.refresh();
        toast.success(t("admin.frpTunnel.logsCleared"));
      },
    });
  };

  const startInstance = async (instance: FrpcInstanceStatus) => {
    if (startingInstanceId.value) return;
    startingInstanceId.value = instance.id;
    try {
      await FrpcAPI.startInstance(instance.id);
      await selectFrpAsDefaultTunnel();
      toast.success(t("admin.frpTunnel.startSuccess"));
      await loadStatus();
    } catch (error) {
      toast.error(t("admin.frpTunnel.startFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.startFailed"),
        ),
      });
    } finally {
      startingInstanceId.value = null;
    }
  };

  const stopInstance = async (instance: FrpcInstanceStatus) => {
    if (stoppingInstanceId.value) return;
    stoppingInstanceId.value = instance.id;
    try {
      await FrpcAPI.stopInstance(instance.id);
      toast.success(t("admin.frpTunnel.stopSuccess"));
      await loadStatus();
    } catch (error) {
      toast.error(t("admin.frpTunnel.stopFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.stopFailed"),
        ),
      });
    } finally {
      stoppingInstanceId.value = null;
    }
  };

  const deleteInstance = async (instance: FrpcInstanceStatus) => {
    if (deletingInstanceId.value) return;
    deletingInstanceId.value = instance.id;
    try {
      await FrpcAPI.deleteInstance(instance.id);
      toast.success(t("admin.frpTunnel.instanceDeleted"));
      await loadStatus();
    } catch (error) {
      toast.error(t("admin.frpTunnel.deleteFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpTunnel.deleteFailed"),
        ),
      });
    } finally {
      deletingInstanceId.value = null;
    }
  };

  const gotoFrpResources = () => {
    showInitDialog.value = false;
    void router.push({ path: "/system", query: { tab: "frp" } });
  };

  function handleStartFailureLogs(lines: string[]) {
    const trace = startErrorTrace.value;
    if (!trace) return;
    if (Date.now() > trace.expireAt) {
      startErrorTrace.value = null;
      return;
    }
    for (const line of lines) {
      const text = line.trim();
      if (!text) continue;
      if (!trace.markerSeen && text.includes(`frpc started pid=${trace.pid}`)) {
        trace.markerSeen = true;
        continue;
      }
      if (!trace.markerSeen) continue;
      if (!CONNECTION_REFUSED_REGEX.test(text)) continue;
      toast.error(t("admin.frpTunnel.startFailed"), {
        description: t("admin.frpTunnel.connectionRefused"),
      });
      startErrorTrace.value = null;
      return;
    }
  }

  onMounted(async () => {
    await loadStatus();
    await loadConfig();
    frpcPolling.start();
  });
  onUnmounted(() => frpcPolling.stop());

  return {
    canStart,
    canStop,
    configLoaded,
    defaults,
    deleteInstance,
    deletingInstanceId,
    extraInstances,
    formatSummary,
    getInstanceDisplayName,
    gotoFrpResources,
    gotoInstanceCreate,
    gotoInstanceDetail,
    isClearingLogs,
    isSaving,
    isStarting,
    isStopping,
    onClearLogsClick,
    overview,
    pid,
    primaryConfig,
    primaryInstance,
    primaryLogs,
    primarySummary,
    running,
    saveConfig,
    setPrimaryEditorRef,
    showInitDialog,
    startFrpc,
    startInstance,
    startingInstanceId,
    stopFrpc,
    stopInstance,
    stoppingInstanceId,
    t,
  };
};
