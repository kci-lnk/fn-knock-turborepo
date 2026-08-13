import { ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { DDNSAPI } from "@/lib/api/ddns";

type Translate = (key: string) => string;

interface UseDDNSPrimaryConfigActionsOptions {
  loadConfig: () => Promise<unknown>;
  loadStatus: () => Promise<unknown>;
  normalizeForSubmit: () => void;
  providerConfig: Ref<Record<string, string>>;
  refreshPolling: () => void;
  resetFieldEditReady: () => void;
  savedProvider: Ref<string>;
  savedProviderConfig: Ref<Record<string, string>>;
  selectedProvider: Ref<string>;
  translate: Translate;
  validate: () => boolean;
}

export function useDDNSPrimaryConfigActions({
  loadConfig,
  loadStatus,
  normalizeForSubmit,
  providerConfig,
  refreshPolling,
  resetFieldEditReady,
  savedProvider,
  savedProviderConfig,
  selectedProvider,
  translate,
  validate,
}: UseDDNSPrimaryConfigActionsOptions) {
  const showClearPrimaryConfigDialog = ref(false);
  const pendingPrimaryConfigCollapse = ref<(() => void) | null>(null);

  const { isPending: isSaving, run: runSaveConfig } = useAsyncAction({
    rethrow: true,
    onError: (error) => {
      toast.error(translate("admin.ddns.saveConfigFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.ddns.saveConfigFailed"),
        ),
      });
    },
  });
  const { isPending: isClearingPrimaryConfig, run: runClearPrimaryConfig } =
    useAsyncAction({
      onError: (error) => {
        toast.error(translate("admin.ddns.clearPrimaryConfigFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.ddns.clearPrimaryConfigFailed"),
          ),
        });
      },
    });

  async function onSaveConfigSilent() {
    if (!selectedProvider.value) return false;
    normalizeForSubmit();
    if (!validate()) return false;

    const provider = selectedProvider.value;
    try {
      await runSaveConfig(async () => {
        await DDNSAPI.saveConfig(provider, providerConfig.value);
        if (provider !== savedProvider.value) {
          await DDNSAPI.setProvider(provider);
        }
      });
      savedProvider.value = provider;
      savedProviderConfig.value = { ...providerConfig.value };
      await loadStatus();
      await loadConfig();
      return true;
    } catch {
      return false;
    }
  }

  async function onSaveConfig() {
    const saved = await onSaveConfigSilent();
    if (!saved) return;
    toast.success(translate("admin.ddns.configSaved"));
  }

  async function onCancelPrimaryConfigEdit() {
    selectedProvider.value = savedProvider.value;
    await loadConfig();
    toast.info(translate("admin.ddns.configChangesDiscarded"));
  }

  function openClearPrimaryConfigDialog(collapse: () => void) {
    pendingPrimaryConfigCollapse.value = collapse;
    showClearPrimaryConfigDialog.value = true;
  }

  async function confirmClearPrimaryConfig() {
    if (!selectedProvider.value) return;

    await runClearPrimaryConfig(
      () => DDNSAPI.saveConfig(selectedProvider.value, {}),
      {
        onSuccess: async () => {
          providerConfig.value = {};
          savedProviderConfig.value = {};
          resetFieldEditReady();
          showClearPrimaryConfigDialog.value = false;
          pendingPrimaryConfigCollapse.value?.();
          pendingPrimaryConfigCollapse.value = null;
          await loadStatus();
          await loadConfig();
          refreshPolling();
          toast.success(translate("admin.ddns.primaryConfigCleared"));
        },
      },
    );
  }

  return {
    confirmClearPrimaryConfig,
    isClearingPrimaryConfig,
    isSaving,
    onCancelPrimaryConfigEdit,
    onSaveConfig,
    onSaveConfigSilent,
    openClearPrimaryConfigDialog,
    showClearPrimaryConfigDialog,
  };
}
