import { computed, nextTick, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRoute, useRouter } from "vue-router";
import { ConfigAPI } from "@/lib/api/config";
import {
  FrpcAPI,
  type FrpcInstanceStatus,
  type FrpcInstanceSummary,
} from "@/lib/api/tunnel";
import { toast } from "@admin-shared/utils/toast";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import {
  DEFAULT_LOG_WINDOW_SIZE,
  mergePollingLogWindow,
} from "@admin-shared/utils/log-window";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";
import { useConfigStore } from "../../../store/config";
import { summarizeFrpcContent } from "./frpcInstanceModel";

export type FrpcEditorExpose = {
  getContent: () => string;
  resetFromRaw: (raw: string) => void;
};

export function useFrpcInstancePage() {
  let isDisposed = false;
  const route = useRoute();
  const router = useRouter();
  const configStore = useConfigStore();
  const { t } = useI18n();

  const instanceId = computed(() => String(route.params.id || ""));
  const isCreateMode = computed(() => route.name === "FrpcInstanceCreate");
  const defaults = ref<{ local_port: string }>({ local_port: "7999" });
  const instance = ref<FrpcInstanceStatus | null>(null);
  const content = ref("");
  const name = ref("");
  const logs = ref<string[]>([]);
  const cursor = ref<number | undefined>(undefined);
  const editorRef = ref<FrpcEditorExpose | null>(null);
  const configSectionRef = ref<HTMLElement | null>(null);
  const isLoading = ref(false);
  const isSaving = ref(false);
  const isStarting = ref(false);
  const isStopping = ref(false);
  const isClearingLogs = ref(false);

  const getInstanceDisplayName = (
    value: FrpcInstanceStatus | null | undefined,
  ) => {
    if (!value) return t("admin.frpcInstancePage.frpInstance");
    const displayName = value.name.trim();
    if (displayName) return displayName;
    if (value.summary.serverAddr) {
      return `${value.summary.serverAddr}:${value.summary.serverPort || "7000"}`;
    }
    return value.isPrimary
      ? t("admin.frpcInstancePage.primaryFrp")
      : t("admin.frpcInstancePage.frpInstance");
  };

  const title = computed(() =>
    isCreateMode.value
      ? t("admin.frpcInstancePage.newFrp")
      : getInstanceDisplayName(instance.value),
  );
  const summary = computed(
    () =>
      instance.value?.summary ??
      summarizeFrpcContent(content.value, defaults.value.local_port),
  );
  const shouldOpenLogs = computed(() => route.query.section === "logs");
  const shouldOpenConfig = computed(() => route.query.section === "config");

  const setConfigSectionRef = (value: unknown) => {
    configSectionRef.value = value instanceof HTMLElement ? value : null;
  };

  const setEditorRef = (value: unknown) => {
    editorRef.value = value as FrpcEditorExpose | null;
  };

  const formatSummary = (value: FrpcInstanceSummary) => {
    const server = value.serverAddr
      ? `${value.serverAddr}:${value.serverPort || "7000"}`
      : t("admin.frpcInstancePage.notConfigured");
    const local = value.localPort || defaults.value.local_port;
    const remote = value.remotePort || "0";
    return t("admin.frpcInstancePage.summary", { server, local, remote });
  };

  const backToList = () => {
    void router.push({ path: "/tunnel", query: { tab: "frp" } });
  };

  const restoreInitialScrollPosition = async () => {
    await nextTick();
    await new Promise<void>((resolve) => {
      window.requestAnimationFrame(() => resolve());
    });
    if (shouldOpenLogs.value) {
      window.scrollTo({
        top: document.documentElement.scrollHeight,
        left: 0,
      });
      return;
    }
    if (shouldOpenConfig.value && configSectionRef.value) {
      configSectionRef.value.scrollIntoView({ block: "start" });
      return;
    }
    window.scrollTo({ top: 0, left: 0 });
  };

  const loadDefaults = async () => {
    const overview = await FrpcAPI.getInstances();
    defaults.value = overview.defaults;
    if (!isCreateMode.value) {
      const next = overview.items.find((item) => item.id === instanceId.value);
      if (next) instance.value = next;
    }
  };

  const stopPolling = () => {
    logsPoller.stop();
  };

  const pollLogs = async () => {
    if (isCreateMode.value || !instance.value) return;
    try {
      const payload = await FrpcAPI.pollInstance(instance.value.id, cursor.value);
      cursor.value = payload.cursor;
      logs.value = mergePollingLogWindow(logs.value, payload.logs, {
        reset: payload.reset,
        max: DEFAULT_LOG_WINDOW_SIZE,
      });
      instance.value = payload.status;
    } catch {
      stopPolling();
    }
  };

  const logsPoller = createVisibilityPoller({
    intervalMs: 2_000,
    enabled: () => !isCreateMode.value && instance.value !== null,
    task: pollLogs,
  });

  const startPolling = () => {
    cursor.value = undefined;
    logsPoller.start();
    logsPoller.sync();
  };

  const loadPage = async () => {
    isLoading.value = true;
    try {
      await loadDefaults();
      if (isCreateMode.value) {
        content.value = await FrpcAPI.createDraft();
        name.value = "";
        logs.value = [];
        editorRef.value?.resetFromRaw(content.value);
        await restoreInitialScrollPosition();
        return;
      }

      const detail = await FrpcAPI.getInstance(instanceId.value);
      instance.value = detail.item;
      content.value = detail.content;
      name.value = detail.item.name;
      logs.value = detail.logs;
      cursor.value = undefined;
      editorRef.value?.resetFromRaw(detail.content);
      if (!isDisposed) startPolling();
      await restoreInitialScrollPosition();
    } catch (error) {
      toast.error(t("admin.frpcInstancePage.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpcInstancePage.loadFailed"),
        ),
      });
    } finally {
      isLoading.value = false;
    }
  };

  const saveInstance = async () => {
    if (isSaving.value) return;
    isSaving.value = true;
    try {
      const nextContent = editorRef.value?.getContent() ?? content.value;
      if (isCreateMode.value) {
        const created = await FrpcAPI.createInstance({
          name: name.value.trim(),
          content: nextContent,
        });
        toast.success(t("admin.frpcInstancePage.created"));
        await router.replace({
          path: `/tunnel/frp/instances/${encodeURIComponent(created.id)}`,
        });
        await loadPage();
        return;
      }

      if (!instance.value) return;
      const wasRunning = instance.value.desiredRunning;
      const updated = await FrpcAPI.updateInstance(instance.value.id, {
        name: name.value.trim(),
        content: nextContent,
      });
      instance.value = updated;
      content.value = nextContent;
      toast.success(
        t(
          wasRunning
            ? "admin.frpcInstancePage.savedAndRestarted"
            : "admin.frpcInstancePage.saved",
        ),
      );
    } catch (error) {
      toast.error(t("admin.frpcInstancePage.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpcInstancePage.saveFailed"),
        ),
      });
    } finally {
      isSaving.value = false;
    }
  };

  const startInstance = async () => {
    if (!instance.value || isStarting.value) return;
    isStarting.value = true;
    try {
      await FrpcAPI.startInstance(instance.value.id);
      await ConfigAPI.updateDefaultTunnel("frp");
      if (configStore.config) {
        configStore.config.default_tunnel = "frp";
      }
      toast.success(t("admin.frpcInstancePage.startSuccess"));
      await loadPage();
    } catch (error) {
      toast.error(t("admin.frpcInstancePage.startFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpcInstancePage.startFailed"),
        ),
      });
    } finally {
      isStarting.value = false;
    }
  };

  const stopInstance = async () => {
    if (!instance.value || isStopping.value) return;
    isStopping.value = true;
    try {
      await FrpcAPI.stopInstance(instance.value.id);
      toast.success(t("admin.frpcInstancePage.stopSuccess"));
      await loadPage();
    } catch (error) {
      toast.error(t("admin.frpcInstancePage.stopFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpcInstancePage.stopFailed"),
        ),
      });
    } finally {
      isStopping.value = false;
    }
  };

  const clearLogs = async () => {
    if (!instance.value || isClearingLogs.value) return;
    isClearingLogs.value = true;
    try {
      await FrpcAPI.clearInstanceLogs(instance.value.id);
      logs.value = [];
      cursor.value = undefined;
      toast.success(t("admin.frpcInstancePage.logsCleared"));
    } catch (error) {
      toast.error(t("admin.frpcInstancePage.clearLogsFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.frpcInstancePage.clearLogsFailed"),
        ),
      });
    } finally {
      isClearingLogs.value = false;
    }
  };

  onMounted(() => {
    void loadPage();
  });

  onUnmounted(() => {
    isDisposed = true;
    stopPolling();
  });

  return {
    backToList,
    clearLogs,
    content,
    defaults,
    formatSummary,
    instance,
    instanceId,
    isClearingLogs,
    isCreateMode,
    isLoading,
    isSaving,
    isStarting,
    isStopping,
    logs,
    name,
    saveInstance,
    setConfigSectionRef,
    setEditorRef,
    startInstance,
    stopInstance,
    summary,
    title,
  };
}
