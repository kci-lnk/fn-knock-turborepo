import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  PanelSyncAPI,
  type PanelConnection,
  type PanelConnectionInput,
  type PanelConnectionUpdateInput,
  type PanelProviderDescriptor,
  type PanelSyncPreview,
} from "@/lib/api/panel-sync-api";

export const usePanelSyncConnections = () => {
  const { t } = useI18n();
  const providers = ref<PanelProviderDescriptor[]>([]);
  const connections = ref<PanelConnection[]>([]);
  const loading = ref(false);
  const saving = ref(false);
  const testingIds = ref(new Set<string>());
  const deletingIds = ref(new Set<string>());
  const previewingCleanupIds = ref(new Set<string>());

  const load = async () => {
    loading.value = true;
    try {
      [providers.value, connections.value] = await Promise.all([
        PanelSyncAPI.providers(),
        PanelSyncAPI.connections(),
      ]);
    } catch (error) {
      toast.error(t("admin.panelSync.messages.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.loadFailed"),
        ),
      });
    } finally {
      loading.value = false;
    }
  };

  const create = async (input: PanelConnectionInput) => {
    saving.value = true;
    try {
      const created = await PanelSyncAPI.create(input);
      toast.success(t("admin.panelSync.messages.created"));
      await load();
      return created;
    } catch (error) {
      toast.error(t("admin.panelSync.messages.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.saveFailed"),
        ),
      });
      return null;
    } finally {
      saving.value = false;
    }
  };

  const update = async (id: string, input: PanelConnectionUpdateInput) => {
    saving.value = true;
    try {
      const updated = await PanelSyncAPI.update(id, input);
      toast.success(t("admin.panelSync.messages.updated"));
      await load();
      return updated;
    } catch (error) {
      toast.error(t("admin.panelSync.messages.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.saveFailed"),
        ),
      });
      return null;
    } finally {
      saving.value = false;
    }
  };

  const verifySaved = async (id: string, notify = true) => {
    testingIds.value = new Set(testingIds.value).add(id);
    try {
      const result = await PanelSyncAPI.testSaved(id);
      if (notify) {
        toast.success(t("admin.panelSync.messages.testSuccess"), {
          description: result.version
            ? `${result.message} · ${result.version}`
            : result.message,
        });
      }
      await load();
      return true;
    } catch (error) {
      toast.error(t("admin.panelSync.messages.testFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.testFailed"),
        ),
      });
      return false;
    } finally {
      const next = new Set(testingIds.value);
      next.delete(id);
      testingIds.value = next;
    }
  };

  const testSaved = (connection: PanelConnection) =>
    verifySaved(connection.id);

  const previewCleanup = async (connection: PanelConnection) => {
    previewingCleanupIds.value = new Set(previewingCleanupIds.value).add(
      connection.id,
    );
    try {
      return await PanelSyncAPI.preview(connection.id, true);
    } catch (error) {
      toast.error(t("admin.panelSync.messages.cleanupPreviewFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.cleanupPreviewFailed"),
        ),
      });
      return null;
    } finally {
      const next = new Set(previewingCleanupIds.value);
      next.delete(connection.id);
      previewingCleanupIds.value = next;
    }
  };

  const remove = async (
    connection: PanelConnection,
    cleanupPreview?: PanelSyncPreview,
  ) => {
    deletingIds.value = new Set(deletingIds.value).add(connection.id);
    try {
      await PanelSyncAPI.remove(connection.id, cleanupPreview);
      toast.success(t("admin.panelSync.messages.deleted"), {
        description: cleanupPreview
          ? t("admin.panelSync.remoteCleaned")
          : t("admin.panelSync.remoteRetained"),
      });
      await load();
    } catch (error) {
      toast.error(t("admin.panelSync.messages.deleteFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.panelSync.messages.deleteFailed"),
        ),
      });
    } finally {
      const next = new Set(deletingIds.value);
      next.delete(connection.id);
      deletingIds.value = next;
    }
  };

  onMounted(load);

  return {
    connections,
    create,
    deletingIds,
    load,
    loading,
    providers,
    previewCleanup,
    previewingCleanupIds,
    remove,
    saving,
    testSaved,
    testingIds,
    update,
    verifySaved,
  };
};
