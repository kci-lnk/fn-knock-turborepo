import { onBeforeUnmount, ref } from "vue";
import { useI18n } from "vue-i18n";
import axios from "axios";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  PanelSyncAPI,
  type PanelConnection,
  type PanelSyncPreview,
  type PanelSyncRun,
} from "@/lib/api/panel-sync-api";

export const usePanelSyncRun = (reloadConnections: () => Promise<void>) => {
  const { t } = useI18n();
  const previewOpen = ref(false);
  const previewingId = ref("");
  const preview = ref<PanelSyncPreview | null>(null);
  const previewConnection = ref<PanelConnection | null>(null);
  const syncing = ref(false);
  const historyOpen = ref(false);
  const historyConnection = ref<PanelConnection | null>(null);
  const history = ref<PanelSyncRun[]>([]);
  const loadingHistory = ref(false);
  let pollTimer: ReturnType<typeof setTimeout> | undefined;

  const openPreview = async (connection: PanelConnection) => {
    previewingId.value = connection.id;
    previewConnection.value = connection;
    try {
      preview.value = await PanelSyncAPI.preview(connection.id);
      previewOpen.value = true;
    } catch (error) {
      toast.error(t("admin.panelSync.messages.previewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.previewFailed"),
        ),
      });
    } finally {
      previewingId.value = "";
    }
  };

  const pollRun = async (runId: string) => {
    try {
      const run = await PanelSyncAPI.run(runId);
      if (["queued", "running"].includes(run.status)) {
        pollTimer = setTimeout(() => void pollRun(runId), 1200);
        return;
      }
      syncing.value = false;
      await reloadConnections();
      if (run.status === "success" || run.status === "skipped") {
        toast.success(t("admin.panelSync.messages.syncSuccess"));
        previewOpen.value = false;
      } else {
        toast.error(t("admin.panelSync.messages.syncFailed"), {
          description: run.message ?? undefined,
        });
      }
    } catch (error) {
      syncing.value = false;
      toast.error(t("admin.panelSync.messages.syncFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.syncFailed"),
        ),
      });
    }
  };

  const confirmSync = async () => {
    if (!preview.value || !previewConnection.value) return;
    syncing.value = true;
    try {
      const accepted = await PanelSyncAPI.sync(
        previewConnection.value.id,
        preview.value,
      );
      await pollRun(accepted.run_id);
    } catch (error) {
      syncing.value = false;
      const message = extractErrorMessage(
        error,
        t("admin.panelSync.messages.syncFailed"),
      );
      const planChanged = axios.isAxiosError(error) && error.response?.status === 409;
      toast.error(
        planChanged
          ? t("admin.panelSync.messages.planChanged")
          : t("admin.panelSync.messages.syncFailed"),
        { description: message },
      );
      if (planChanged) {
        await openPreview(previewConnection.value);
      }
    }
  };

  const openHistory = async (connection: PanelConnection) => {
    historyConnection.value = connection;
    historyOpen.value = true;
    loadingHistory.value = true;
    try {
      history.value = await PanelSyncAPI.runs(connection.id);
    } catch (error) {
      toast.error(t("admin.panelSync.messages.historyFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.historyFailed"),
        ),
      });
    } finally {
      loadingHistory.value = false;
    }
  };

  onBeforeUnmount(() => {
    if (pollTimer) clearTimeout(pollTimer);
  });

  return {
    confirmSync,
    history,
    historyConnection,
    historyOpen,
    loadingHistory,
    openHistory,
    openPreview,
    preview,
    previewConnection,
    previewOpen,
    previewingId,
    syncing,
  };
};
