import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { SystemAPI } from "@/lib/api";
import { useConfigStore } from "@/store/config";
import type { FirewallAdditionalPortsDetails, RunType } from "@/types";
import { resolveFirewallAdditionalPortsSuccessMessageKey } from "./firewallAdditionalPortsModel";

type UseFirewallAdditionalPortsOptions = {
  canManageHostFirewall: () => boolean;
  hasUnsavedModeChanges: () => boolean;
};

type FirewallAdditionalPortsControllerDependencies = {
  getDetails: () => Promise<FirewallAdditionalPortsDetails>;
  onLoadError: (error: unknown) => void;
  onSaved: (
    result: FirewallAdditionalPortsDetails,
    showUnsavedModeNotice: boolean,
  ) => void;
  onSaveError: (error: unknown) => void;
  onUnsupported: () => void;
  onUpdated: (result: FirewallAdditionalPortsDetails) => void;
  updatePorts: (ports: number[]) => Promise<FirewallAdditionalPortsDetails>;
};

export const createFirewallAdditionalPortsController = (
  { canManageHostFirewall, hasUnsavedModeChanges }: UseFirewallAdditionalPortsOptions,
  dependencies: FirewallAdditionalPortsControllerDependencies,
) => {
  const open = ref(false);
  const details = ref<FirewallAdditionalPortsDetails | null>(null);
  const loadFailed = ref(false);

  const { isPending: loading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      loadFailed.value = true;
      dependencies.onLoadError(error);
    },
  });
  const { isPending: saving, run: runSave } = useAsyncAction({
    onError: dependencies.onSaveError,
  });

  const load = async () => {
    loadFailed.value = false;
    await runLoad(async () => {
      details.value = await dependencies.getDetails();
    });
  };
  const openDialog = async () => {
    if (!canManageHostFirewall()) {
      dependencies.onUnsupported();
      return;
    }
    details.value = null;
    loadFailed.value = false;
    open.value = true;
    await load();
  };
  const updateOpen = (nextOpen: boolean) => {
    if (saving.value) return;
    open.value = nextOpen;
  };
  const save = async (ports: number[]) => {
    const showUnsavedModeNotice = hasUnsavedModeChanges();
    await runSave(async () => {
      const result = await dependencies.updatePorts(ports);
      details.value = result;
      dependencies.onUpdated(result);
      open.value = false;
      dependencies.onSaved(result, showUnsavedModeNotice);
    });
  };

  return {
    details,
    load,
    loadFailed,
    loading,
    open,
    openDialog,
    save,
    saving,
    updateOpen,
  };
};

export const useFirewallAdditionalPorts = ({
  canManageHostFirewall,
  hasUnsavedModeChanges,
}: UseFirewallAdditionalPortsOptions) => {
  const configStore = useConfigStore();
  const { locale, t } = useI18n();
  const autoManageFirewallEnabled = computed(
    () => configStore.config?.auto_manage_firewall !== false,
  );

  const modeLabel = (runType: RunType) => {
    if (runType === 0) return t("admin.runModeSettings.directModeName");
    if (runType === 1) {
      return t("admin.runModeSettings.additionalPorts.reverseModeName");
    }
    return t("admin.runModeSettings.subdomainModeName");
  };
  const formatPorts = (ports: number[]) =>
    ports.length
      ? ports.join(locale.value === "en" ? ", " : "、")
      : t("admin.runModeSettings.additionalPorts.noPorts");
  const errorDescription = (error: unknown) =>
    extractErrorMessage(error, t("admin.runModeSettings.operationFailed"));

  const controller = createFirewallAdditionalPortsController(
    { canManageHostFirewall, hasUnsavedModeChanges },
    {
      getDetails: () => SystemAPI.getFirewallAdditionalPorts(),
      updatePorts: (ports) => SystemAPI.updateFirewallAdditionalPorts(ports),
      onLoadError: (error) => {
        toast.error(t("admin.runModeSettings.additionalPorts.loadFailed"), {
          description: errorDescription(error),
        });
      },
      onSaveError: (error) => {
        toast.error(t("admin.runModeSettings.additionalPorts.saveFailed"), {
          description: errorDescription(error),
        });
      },
      onUnsupported: () => {
        toast.error(t("admin.runModeSettings.firewallUnsupportedTitle"));
      },
      onUpdated: (result) => {
        if (configStore.config) {
          configStore.config.firewall_additional_ports = result.additionalPorts;
        }
      },
      onSaved: (result, showUnsavedModeNotice) => {
        const successMessageKey =
          resolveFirewallAdditionalPortsSuccessMessageKey(
            result,
            autoManageFirewallEnabled.value,
          );
        const baseDescription = result.appliedNow
          ? t(
              `admin.runModeSettings.additionalPorts.${successMessageKey}`,
              {
                count: result.additionalPorts.length,
                mode: modeLabel(result.runType),
                ports: formatPorts(result.effectivePorts),
              },
            )
          : t(
              `admin.runModeSettings.additionalPorts.${successMessageKey}`,
              {
                count: result.additionalPorts.length,
              },
            );
        toast.success(t("admin.runModeSettings.additionalPorts.saved"), {
          description: showUnsavedModeNotice
            ? `${baseDescription} ${t("admin.runModeSettings.additionalPorts.savedModeNotice")}`
            : baseDescription,
        });
      },
    },
  );

  return {
    ...controller,
    autoManageFirewallEnabled,
    hasUnsavedModeChanges: computed(() => hasUnsavedModeChanges()),
    modeLabel: computed(() =>
      controller.details.value
        ? modeLabel(controller.details.value.runType)
        : "",
    ),
  };
};
