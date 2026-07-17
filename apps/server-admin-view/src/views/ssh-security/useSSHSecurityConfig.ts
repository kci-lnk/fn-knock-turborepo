import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { SSHSecurityAPI } from "@/lib/api";
import { useConfigStore } from "@/store/config";
import type { SSHSecurityDetails, SSHSecuritySelection } from "@/types";

type SSHBlockListPanelInstance = {
  loadBlocks: () => Promise<void>;
};

export const useSSHSecurityConfig = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const details = ref<SSHSecurityDetails | null>(null);
  const activeTab = ref("login-logs");
  const isClearFirewallDialogOpen = ref(false);
  const blockListPanel = ref<SSHBlockListPanelInstance | null>(null);
  const setBlockListPanel = (panel: unknown) => {
    blockListPanel.value = panel as SSHBlockListPanelInstance | null;
  };

  const form = reactive({
    enabled: false,
    windowMinutes: 10,
    failedLoginThreshold: 5,
    blockDurationValue: 1,
    blockDurationUnit: "day" as "minute" | "hour" | "day",
    allowedRegions: [] as SSHSecuritySelection[],
    customCidrsText: "",
  });

  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sshSecurity.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sshSecurity.loadDescription"),
        ),
      });
    },
  });
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sshSecurity.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sshSecurity.saveDescription"),
        ),
      });
    },
  });
  const { isPending: isSyncingFirewall, run: runSyncFirewall } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.sshSecurity.syncFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.sshSecurity.syncDescription"),
          ),
        });
      },
    });

  const customCidrsState = computed(() =>
    parseCidrTextarea(form.customCidrsText),
  );
  const invalidCustomCidrs = computed(() => customCidrsState.value.invalid);
  const regionInputsDisabled = computed(() => isSaving.value || !form.enabled);
  const sshPortsLabel = computed(() => {
    const ports = details.value?.summary.ssh_ports ?? [22];
    return ports.length > 0
      ? ports.join(t("admin.sshSecurity.listSeparator"))
      : "22";
  });
  const summaryText = computed(() => {
    const summary = details.value?.summary;
    if (!summary) return t("admin.sshSecurity.notLoaded");
    return t("admin.sshSecurity.summary", {
      status: summary.enabled
        ? t("admin.sshSecurity.enabled")
        : t("admin.sshSecurity.disabled"),
      ports: sshPortsLabel.value,
      allowed: summary.allowed_cidr_count,
      blocks: summary.active_block_count,
    });
  });
  const saveBlockedReason = computed(() => {
    if (!details.value?.summary.available && form.enabled) {
      return (
        details.value?.summary.unavailable_reason ||
        t("admin.sshSecurity.unavailableToEnable")
      );
    }
    if (invalidCustomCidrs.value.length > 0) {
      return t("admin.sshSecurity.fixCustomCidrs");
    }
    return "";
  });

  const applyDetails = (value: SSHSecurityDetails) => {
    details.value = value;
    form.enabled = value.config.enabled;
    form.windowMinutes = value.config.window_minutes;
    form.failedLoginThreshold = value.config.failed_login_threshold;
    form.blockDurationValue = value.config.block_duration_value;
    form.blockDurationUnit = value.config.block_duration_unit;
    form.allowedRegions = value.config.allowed_regions.map((item) => ({
      ...item,
    }));
    form.customCidrsText = value.config.custom_cidrs.join("\n");
  };

  const loadDetails = async () => {
    await runLoad(async () => {
      applyDetails(await SSHSecurityAPI.getDetails());
    });
  };
  const reloadBlockList = () =>
    blockListPanel.value?.loadBlocks() ?? Promise.resolve();

  const saveConfig = async () => {
    if (saveBlockedReason.value) {
      toast.error(t("admin.sshSecurity.cannotSave"), {
        description: saveBlockedReason.value,
      });
      return;
    }
    await runSave(
      () =>
        SSHSecurityAPI.updateConfig({
          enabled: form.enabled,
          window_minutes: form.windowMinutes,
          failed_login_threshold: form.failedLoginThreshold,
          block_duration_value: form.blockDurationValue,
          block_duration_unit: form.blockDurationUnit,
          allowed_regions: form.allowedRegions.map((item) => ({
            province: item.province,
            query_city: item.query_city,
            operator: item.operator,
          })),
          custom_cidrs: customCidrsState.value.cidrs,
        }),
      {
        onSuccess: async (nextDetails) => {
          applyDetails(nextDetails);
          toast.success(t("admin.sshSecurity.saved"));
          await configStore.loadConfig();
        },
      },
    );
  };

  const syncFirewall = async () => {
    if (!details.value?.summary.available) {
      toast.error(t("admin.sshSecurity.cannotSync"), {
        description:
          details.value?.summary.unavailable_reason ||
          t("admin.sshSecurity.unavailableToSync"),
      });
      return;
    }
    await runSyncFirewall(SSHSecurityAPI.syncFirewall, {
      onSuccess: async (result) => {
        toast.success(t("admin.sshSecurity.firewallSynced"), {
          description: t("admin.sshSecurity.firewallSyncedDescription", {
            allowed: result.allowed_cidrs,
            synced: result.synced,
            ports:
              result.ports.join(t("admin.sshSecurity.listSeparator")) || "22",
          }),
        });
        await Promise.all([loadDetails(), reloadBlockList()]);
      },
    });
  };

  const openClearFirewallDialog = () => {
    if (!details.value?.summary.available || isSyncingFirewall.value) return;
    isClearFirewallDialogOpen.value = true;
  };
  const clearFirewall = async () => {
    if (!details.value?.summary.available) {
      toast.error(t("admin.sshSecurity.cannotClear"), {
        description:
          details.value?.summary.unavailable_reason ||
          t("admin.sshSecurity.unavailableToClear"),
      });
      return;
    }
    await runSyncFirewall(SSHSecurityAPI.clearFirewall, {
      onSuccess: async (result) => {
        isClearFirewallDialogOpen.value = false;
        toast.success(t("admin.sshSecurity.firewallCleared"), {
          description: t("admin.sshSecurity.firewallClearedDescription", {
            count: result.cleared_blocks,
          }),
        });
        await Promise.all([loadDetails(), reloadBlockList()]);
      },
    });
  };

  onMounted(() => {
    void loadDetails();
  });

  return {
    activeTab,
    clearFirewall,
    customCidrsState,
    details,
    form,
    invalidCustomCidrs,
    isClearFirewallDialogOpen,
    isLoading,
    isSaving,
    isSyncingFirewall,
    loadDetails,
    openClearFirewallDialog,
    regionInputsDisabled,
    saveBlockedReason,
    saveConfig,
    setBlockListPanel,
    sshPortsLabel,
    summaryText,
    syncFirewall,
    t,
  };
};
