import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { WAFAPI } from "../../../lib/api";
import type { WAFDetails } from "../../../types";
import { useConfigStore } from "../../../store/config";
import { useWAFRuleManagement } from "./useWAFRuleManagement";

const clampLevel = (value: unknown, fallback = 1) => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(4, Math.max(1, parsed));
};

export function useWAFSettings() {
  const { locale, t } = useI18n();
  const configStore = useConfigStore();
  const details = ref<WAFDetails | null>(null);
  const selectedSystemRules = ref<string[]>([]);
  const selectedCustomRules = ref<string[]>([]);
  const form = reactive({
    enabled: false,
    system_rules_auto_update_enabled: true,
    common_location_exempt_enabled: false,
    paranoia_level: 1,
    executing_paranoia_level: 1,
  });

  const levelOptions = computed(
    () =>
      [
        {
          value: "1",
          label: t("admin.wafSettings.levels.daily"),
          description: t("admin.wafSettings.levels.dailyDescription"),
        },
        {
          value: "2",
          label: t("admin.wafSettings.levels.enhanced"),
          description: t("admin.wafSettings.levels.enhancedDescription"),
        },
        {
          value: "3",
          label: t("admin.wafSettings.levels.strict"),
          description: t("admin.wafSettings.levels.strictDescription"),
        },
        {
          value: "4",
          label: t("admin.wafSettings.levels.maximum"),
          description: t("admin.wafSettings.levels.maximumDescription"),
        },
      ] as const,
  );

  const { isPending: isLoading, run: runLoadDetails } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.wafSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.loadDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.wafSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wafSettings.saveDescription"),
        ),
      });
    },
  });

  const formatDate = (value?: string | null) => {
    if (!value) return "-";
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return date.toLocaleString(locale.value, {
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
    });
  };

  const syncedLabel = computed(() => {
    const syncedAt = details.value?.system.synced?.synced_at;
    return syncedAt ? formatDate(syncedAt) : t("admin.wafSettings.notSynced");
  });
  const manifestLabel = computed(() => {
    const manifest = details.value?.system.manifest;
    if (!manifest) return t("admin.wafSettings.notFetched");
    return manifest.packagingTime
      ? formatDate(manifest.packagingTime)
      : t("admin.wafSettings.fetched");
  });

  const applyFromDetails = (data: WAFDetails) => {
    details.value = data;
    form.enabled = data.config.enabled === true;
    form.system_rules_auto_update_enabled =
      data.config.system_rules_auto_update_enabled !== false;
    form.common_location_exempt_enabled =
      data.config.common_location_exempt_enabled === true;
    const level = clampLevel(data.config.paranoia_level, 1);
    form.paranoia_level = level;
    form.executing_paranoia_level = level;
    selectedSystemRules.value = [];
    selectedCustomRules.value = [];
  };

  const ruleManagement = useWAFRuleManagement({
    applyDetails: applyFromDetails,
    details,
    formatDate,
    selectedCustomRules,
    selectedSystemRules,
  });
  void ruleManagement.uploadInputRef;

  const isBusy = computed(
    () =>
      isSaving.value ||
      ruleManagement.isUpdatingSystemRules.value ||
      ruleManagement.isUploading.value ||
      ruleManagement.isChangingRules.value,
  );

  const fetchDetails = async () => {
    await runLoadDetails(async () => {
      applyFromDetails(await WAFAPI.getDetails());
    });
  };

  const saveSettings = async (
    successMessage = t("admin.wafSettings.settingsUpdated"),
  ) => {
    await runSaveSettings(
      () =>
        WAFAPI.updateConfig({
          enabled: form.enabled,
          system_rules_auto_update_enabled:
            form.system_rules_auto_update_enabled,
          common_location_exempt_enabled:
            form.common_location_exempt_enabled,
          paranoia_level: form.paranoia_level,
          executing_paranoia_level: form.executing_paranoia_level,
        }),
      {
        onSuccess: async (data) => {
          applyFromDetails(data);
          toast.success(successMessage);
          await configStore.loadConfig();
        },
        onError: () => {
          if (details.value) applyFromDetails(details.value);
        },
      },
    );
  };

  const handleParanoiaLevelChange = (value: unknown) => {
    const level = clampLevel(value, 1);
    form.paranoia_level = level;
    form.executing_paranoia_level = level;
    return saveSettings(t("admin.wafSettings.protectionUpdated"));
  };

  const handleEnabledChange = async (enabled: boolean) => {
    if (form.enabled === enabled || isBusy.value) return;
    const previousEnabled = form.enabled;
    form.enabled = enabled;
    await runSaveSettings(
      async () => {
        if (enabled) await ruleManagement.refreshAndSyncSystemRules();
        return WAFAPI.updateConfig({
          enabled,
          system_rules_auto_update_enabled:
            form.system_rules_auto_update_enabled,
          common_location_exempt_enabled:
            form.common_location_exempt_enabled,
          paranoia_level: form.paranoia_level,
          executing_paranoia_level: form.executing_paranoia_level,
        });
      },
      {
        onSuccess: async (data) => {
          applyFromDetails(data);
          toast.success(
            enabled
              ? t("admin.wafSettings.enabledTitle")
              : t("admin.wafSettings.disabledTitle"),
            {
              description: enabled
                ? t("admin.wafSettings.enabledDescription")
                : t("admin.wafSettings.disabledDescription"),
            },
          );
          await configStore.loadConfig();
        },
        onError: () => {
          form.enabled = previousEnabled;
          if (details.value) applyFromDetails(details.value);
        },
      },
    );
  };

  const handleCommonLocationExemptChange = async (enabled: boolean) => {
    if (form.common_location_exempt_enabled === enabled || isBusy.value) return;
    const previousEnabled = form.common_location_exempt_enabled;
    form.common_location_exempt_enabled = enabled;
    await runSaveSettings(
      () => WAFAPI.updateConfig({ common_location_exempt_enabled: enabled }),
      {
        onSuccess: (data) => {
          applyFromDetails(data);
          toast.success(
            enabled
              ? t("admin.wafSettings.commonLocationEnabled")
              : t("admin.wafSettings.commonLocationDisabled"),
          );
        },
        onError: () => {
          form.common_location_exempt_enabled = previousEnabled;
          if (details.value) applyFromDetails(details.value);
        },
      },
    );
  };

  const handleAutoUpdateChange = async (enabled: boolean) => {
    if (form.system_rules_auto_update_enabled === enabled || isBusy.value) {
      return;
    }
    const previousEnabled = form.system_rules_auto_update_enabled;
    form.system_rules_auto_update_enabled = enabled;
    await runSaveSettings(
      () => WAFAPI.updateConfig({ system_rules_auto_update_enabled: enabled }),
      {
        onSuccess: (data) => {
          applyFromDetails(data);
          toast.success(
            enabled
              ? t("admin.wafSettings.autoUpdateEnabled")
              : t("admin.wafSettings.autoUpdateDisabled"),
            {
              description: enabled
                ? t("admin.wafSettings.autoUpdateEnabledDescription")
                : t("admin.wafSettings.autoUpdateDisabledDescription"),
            },
          );
        },
        onError: () => {
          form.system_rules_auto_update_enabled = previousEnabled;
          if (details.value) applyFromDetails(details.value);
        },
      },
    );
  };

  onMounted(fetchDetails);

  return {
    details,
    formatDate,
    form,
    handleAutoUpdateChange,
    handleCommonLocationExemptChange,
    handleEnabledChange,
    handleParanoiaLevelChange,
    isBusy,
    isLoading,
    levelOptions,
    manifestLabel,
    saveSettings,
    selectedCustomRules,
    selectedSystemRules,
    showLoadingSkeleton,
    syncedLabel,
    t,
    ...ruleManagement,
  };
}
