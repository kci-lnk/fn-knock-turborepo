import {
  computed,
  onMounted,
  onUnmounted,
  ref,
  toValue,
  watch,
  type MaybeRefOrGetter,
} from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "vue-sonner";
import { EventCenterAPI } from "@/lib/api/events";
import { RuntimeHealthAPI } from "@/lib/api/runtime-health";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";
import type {
  RuntimeComponentHealth,
  RuntimeHealthSnapshot,
  RuntimeLogComponent,
  RuntimeOperationalLogEntry,
  SystemEventRecord,
} from "@/types";

const componentOrder = [
  "management",
  "gateway_process",
  "gateway_dataplane",
  "auth_bridge",
  "storage",
  "config_sync",
] as const;

export const useRuntimeHealth = (options: {
  active: MaybeRefOrGetter<boolean>;
}) => {
  const { t } = useI18n();
  const snapshot = ref<RuntimeHealthSnapshot | null>(null);
  const recentEvents = ref<SystemEventRecord[]>([]);
  const loading = ref(false);
  const exporting = ref(false);
  const copying = ref(false);
  const logDialogOpen = ref(false);
  const logComponent = ref<RuntimeLogComponent | null>(null);
  const logEntries = ref<RuntimeOperationalLogEntry[]>([]);
  const logGeneratedAt = ref<string | null>(null);
  const logsLoading = ref(false);
  const logsClearing = ref(false);
  const gatewayMemoryDialogOpen = ref(false);
  let logRequestId = 0;

  const components = computed(() =>
    componentOrder
      .map((id) => snapshot.value?.components[id])
      .filter((component): component is RuntimeComponentHealth => !!component),
  );
  const hasProcessDetails = (component: RuntimeComponentHealth) =>
    component.process_state !== "not_applicable";
  const processComponents = computed(() =>
    components.value.filter(hasProcessDetails),
  );
  const serviceComponents = computed(() =>
    components.value.filter((component) => !hasProcessDetails(component)),
  );
  const isLogComponent = (
    component: RuntimeComponentHealth,
  ): component is RuntimeComponentHealth & { id: RuntimeLogComponent } =>
    component.id === "management" || component.id === "gateway_process";
  const selectedLogComponentName = computed(() =>
    logComponent.value
      ? t(`admin.eventCenter.runtime.components.${logComponent.value}`)
      : "",
  );

  const fetchRuntime = async (showError = true, signal?: AbortSignal) => {
    if (loading.value) return;
    loading.value = true;
    try {
      const [health, events] = await Promise.all([
        RuntimeHealthAPI.getHealth(signal),
        EventCenterAPI.getEvents(
          {
            page: 1,
            limit: "20",
            search: "",
            source: "RUNTIME_MONITOR",
          },
          signal,
        ),
      ]);
      if (signal?.aborted) return;
      snapshot.value = health.data;
      recentEvents.value = events.data.events;
    } catch (error) {
      if (signal?.aborted) return;
      if (showError) {
        toast.error(t("admin.eventCenter.runtime.loadFailed"), {
          description: error instanceof Error ? error.message : String(error),
        });
      }
    } finally {
      loading.value = false;
    }
  };

  const runtimePoller = createVisibilityPoller({
    intervalMs: 5_000,
    enabled: () => toValue(options.active),
    task: (signal) => fetchRuntime(snapshot.value === null, signal),
  });

  const copyDiagnostics = async () => {
    copying.value = true;
    try {
      const result = await RuntimeHealthAPI.getDiagnostics();
      await navigator.clipboard.writeText(JSON.stringify(result.data, null, 2));
      toast.success(t("admin.eventCenter.runtime.copySuccess"));
    } catch (error) {
      toast.error(t("admin.eventCenter.runtime.copyFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      copying.value = false;
    }
  };

  const exportDiagnostics = async () => {
    exporting.value = true;
    try {
      const { blob, filename } = await RuntimeHealthAPI.downloadArchive();
      const url = URL.createObjectURL(blob);
      const anchor = document.createElement("a");
      anchor.href = url;
      anchor.download = filename;
      anchor.click();
      URL.revokeObjectURL(url);
      toast.success(t("admin.eventCenter.runtime.exportSuccess"));
    } catch (error) {
      toast.error(t("admin.eventCenter.runtime.exportFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      exporting.value = false;
    }
  };

  const loadRuntimeLogs = async () => {
    const component = logComponent.value;
    if (!component || logsLoading.value) return;
    const requestId = ++logRequestId;
    logsLoading.value = true;
    try {
      const result = await RuntimeHealthAPI.getLogs(component);
      if (requestId !== logRequestId) return;
      logEntries.value = result.data.entries;
      logGeneratedAt.value = result.data.generated_at;
    } catch (error) {
      if (requestId !== logRequestId) return;
      toast.error(t("admin.eventCenter.runtime.logLoadFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      if (requestId === logRequestId) logsLoading.value = false;
    }
  };

  const openRuntimeLogs = (component: RuntimeComponentHealth) => {
    if (!isLogComponent(component)) return;
    logComponent.value = component.id;
    logEntries.value = [];
    logGeneratedAt.value = null;
    logDialogOpen.value = true;
    void loadRuntimeLogs();
  };

  const openGatewayMemoryDialog = (component: RuntimeComponentHealth) => {
    if (component.id === "gateway_process") {
      gatewayMemoryDialogOpen.value = true;
    }
  };

  const clearRuntimeLogs = async () => {
    const component = logComponent.value;
    if (!component || logsClearing.value) return;
    logsClearing.value = true;
    ++logRequestId;
    try {
      const result = await RuntimeHealthAPI.clearLogs(component);
      logEntries.value = [];
      logGeneratedAt.value = result.data.cleared_at;
      toast.success(t("admin.eventCenter.runtime.logClearSuccess"));
      void fetchRuntime(false);
    } catch (error) {
      toast.error(t("admin.eventCenter.runtime.logClearFailed"), {
        description: error instanceof Error ? error.message : String(error),
      });
    } finally {
      logsClearing.value = false;
      logsLoading.value = false;
    }
  };

  watch(() => toValue(options.active), runtimePoller.sync);
  onMounted(runtimePoller.start);
  onUnmounted(runtimePoller.stop);

  return {
    clearRuntimeLogs,
    copying,
    exporting,
    fetchRuntime,
    gatewayMemoryDialogOpen,
    loadRuntimeLogs,
    loading,
    logDialogOpen,
    logEntries,
    logGeneratedAt,
    logsClearing,
    logsLoading,
    openGatewayMemoryDialog,
    openRuntimeLogs,
    processComponents,
    recentEvents,
    selectedLogComponentName,
    serviceComponents,
    snapshot,
    copyDiagnostics,
    exportDiagnostics,
  };
};
