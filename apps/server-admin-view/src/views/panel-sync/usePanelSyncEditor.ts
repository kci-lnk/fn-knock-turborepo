import { computed, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import {
  PanelSyncAPI,
  type PanelConnection,
  type PanelConnectionInput,
  type PanelConnectionUpdateInput,
  type PanelProvider,
} from "@/lib/api/panel-sync-api";
import {
  createPanelSyncForm,
  isPanelAutoSyncReady,
  panelConnectionToForm,
  panelFormToInput,
  panelFormToUpdate,
  nextPanelConnectionName,
  type PanelSyncEditorForm,
} from "./panel-sync-model";

export const usePanelSyncEditor = (
  actions: {
    create: (input: PanelConnectionInput) => Promise<PanelConnection | null>;
    update: (
      id: string,
      input: PanelConnectionUpdateInput,
    ) => Promise<PanelConnection | null>;
    verify: (id: string, notify?: boolean) => Promise<boolean>;
  },
  existingNames: () => Iterable<string>,
) => {
  const { t } = useI18n();
  const open = ref(false);
  const editing = ref<PanelConnection | null>(null);
  const form = reactive<PanelSyncEditorForm>(
    createPanelSyncForm(existingNames()),
  );
  const testing = ref(false);
  const draftVerified = ref(false);
  const isEditing = computed(() => editing.value !== null);
  const autoSyncReady = computed(() =>
    isPanelAutoSyncReady(editing.value, form),
  );

  watch(
    form,
    () => {
      draftVerified.value = false;
    },
    { deep: true },
  );
  watch(
    () => form.clear_credential,
    (clear) => {
      if (clear) {
        form.credential = "";
        form.auto_sync.enabled = false;
      }
    },
  );

  const reset = () => Object.assign(form, createPanelSyncForm(existingNames()));
  const openCreate = () => {
    editing.value = null;
    reset();
    draftVerified.value = false;
    open.value = true;
  };
  const openEdit = (connection: PanelConnection) => {
    editing.value = connection;
    Object.assign(form, panelConnectionToForm(connection));
    draftVerified.value = false;
    open.value = true;
  };

  const selectProvider = (provider: PanelProvider) => {
    if (isEditing.value) return;
    form.provider = provider;
    form.name = nextPanelConnectionName(provider, existingNames());
    form.endpoint_url = "";
    draftVerified.value = false;
  };

  const testDraft = async () => {
    testing.value = true;
    try {
      const result = await PanelSyncAPI.testDraft(
        panelFormToInput(form, editing.value),
        editing.value?.id,
      );
      draftVerified.value = true;
      toast.success(t("admin.panelSync.messages.testSuccess"), {
        description: result.message,
      });
    } catch (error) {
      draftVerified.value = false;
      toast.error(t("admin.panelSync.messages.testFailed"), {
        description:
          error instanceof TypeError
            ? t("admin.panelSync.messages.invalidEndpoint")
            : extractErrorMessage(
                error,
                t("admin.panelSync.messages.testFailed"),
              ),
      });
    } finally {
      testing.value = false;
    }
  };

  const save = async () => {
    try {
      const shouldVerifySavedConnection = draftVerified.value;
      const common = panelFormToUpdate(form, editing.value);
      const saved = editing.value
        ? await actions.update(editing.value.id, common)
        : await actions.create({ ...common, provider: form.provider });
      if (saved && shouldVerifySavedConnection) {
        await actions.verify(saved.id, false);
      }
      if (saved) open.value = false;
    } catch (error) {
      toast.error(t("admin.panelSync.messages.saveFailed"), {
        description:
          error instanceof TypeError
            ? t("admin.panelSync.messages.invalidEndpoint")
            : extractErrorMessage(
                error,
                t("admin.panelSync.messages.saveFailed"),
              ),
      });
    }
  };

  return {
    autoSyncReady,
    draftVerified,
    editing,
    form,
    isEditing,
    open,
    openCreate,
    openEdit,
    save,
    selectProvider,
    testDraft,
    testing,
  };
};
