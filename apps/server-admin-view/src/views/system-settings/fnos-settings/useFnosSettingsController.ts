import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { SystemAPI } from "@/lib/api/system";
import type {
  FnosCertificateSyncDetails,
  FnosNetworkTuningStatus,
  FnosNetworkTuningUpdatePayload,
  FnosPortIconHijackConfig,
  FnosShareBypassConfig,
} from "../../../types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { useConfigStore } from "../../../store/config";
import { useFnosNetworkTuningViewModel } from "./useFnosNetworkTuningViewModel";

const DEFAULT_FNOS_SHARE_BYPASS_VALUES = {
  upstream_timeout_ms: 2_500,
  validation_cache_ttl_seconds: 30,
  validation_lock_ttl_seconds: 5,
  session_ttl_seconds: 300,
} satisfies Omit<FnosShareBypassConfig, "enabled">;

export function useFnosSettingsController() {
  const configStore = useConfigStore();
  const router = useRouter();
  const { t } = useI18n();
  const settings = ref<FnosShareBypassConfig | null>(null);
  const form = reactive<FnosShareBypassConfig>({
    enabled: false,
    ...DEFAULT_FNOS_SHARE_BYPASS_VALUES,
  });
  const iconHijackSettings = ref<FnosPortIconHijackConfig | null>(null);
  const iconHijackForm = reactive<FnosPortIconHijackConfig>({
    enabled: false,
    updated_at: null,
  });
  const networkTuningStatus = ref<FnosNetworkTuningStatus | null>(null);
  const certificateSyncDetails = ref<FnosCertificateSyncDetails | null>(null);
  const networkTuningForm = reactive({
    bbr_enabled: false,
    mtu_probing_enabled: false,
  });

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.fnosSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.fnosSettings.loadDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.fnosSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.fnosSettings.saveDescription"),
        ),
      });
    },
  });
  const { isPending: isIconHijackSaving, run: runSaveIconHijackSettings } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.fnosSettings.saveFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.fnosSettings.saveIconHijackDescription"),
          ),
        });
      },
    });
  const { isPending: isNetworkTuningSaving, run: runSaveNetworkTuningSettings } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.fnosSettings.saveFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.fnosSettings.saveNetworkTuningDescription"),
          ),
        });
      },
    });

  const canUseFnosCertificateSync = computed(
    () => configStore.canUseFnosCertificateSync,
  );
  const canUseFnosConnectWaf = computed(() => configStore.canUseFnosConnectWaf);
  const canUseFnosNetworkTuning = computed(
    () => configStore.canUseFnosNetworkTuning,
  );
  const isShareBypassMode = computed(
    () =>
      configStore.config?.run_type === 1 || configStore.config?.run_type === 3,
  );
  const isRestrictedByRunMode = computed(
    () => configStore.config?.run_type === 0,
  );
  const networkTuningViewModel =
    useFnosNetworkTuningViewModel(networkTuningStatus);

  const applyFromSettings = (data: FnosShareBypassConfig) => {
    settings.value = data;
    Object.assign(form, data);
  };

  const applyIconHijackFromSettings = (data: FnosPortIconHijackConfig) => {
    iconHijackSettings.value = data;
    Object.assign(iconHijackForm, data);
  };

  const applyNetworkTuningFromStatus = (data: FnosNetworkTuningStatus) => {
    networkTuningStatus.value = data;
    networkTuningForm.bbr_enabled = data.config.bbr_enabled;
    networkTuningForm.mtu_probing_enabled = data.config.mtu_probing_enabled;
  };

  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      const [shareBypass, iconHijack, networkTuning] = await Promise.all([
        SystemAPI.getFnosShareBypassConfig(),
        SystemAPI.getFnosPortIconHijackConfig(),
        canUseFnosNetworkTuning.value
          ? SystemAPI.getFnosNetworkTuningStatus()
          : Promise.resolve(null),
      ]);
      applyFromSettings(shareBypass);
      applyIconHijackFromSettings(iconHijack);
      if (networkTuning) {
        applyNetworkTuningFromStatus(networkTuning);
      } else {
        networkTuningStatus.value = null;
      }
      if (canUseFnosCertificateSync.value) {
        try {
          certificateSyncDetails.value =
            await SystemAPI.getFnosCertificateSyncDetails();
        } catch {
          certificateSyncDetails.value = null;
        }
      }
    });
  };

  const saveShareBypassEnabled = async (nextValue: boolean) => {
    if (!isShareBypassMode.value || isSaving.value) {
      if (!isShareBypassMode.value) {
        toast.error(t("admin.fnosSettings.unavailableTitle"), {
          description: t("admin.fnosSettings.unavailableDescription"),
        });
      }
      return;
    }

    const previousSettings = settings.value;
    form.enabled = nextValue;
    const result = await runSaveSettings(
      () =>
        SystemAPI.updateFnosShareBypassConfig({
          enabled: nextValue,
          ...DEFAULT_FNOS_SHARE_BYPASS_VALUES,
        }),
      {
        onSuccess: (data) => {
          applyFromSettings(data);
          toast.success(t("admin.fnosSettings.shareBypassUpdated"));
        },
      },
    );
    if (!result && previousSettings) applyFromSettings(previousSettings);
  };

  const saveIconHijackEnabled = async (nextValue: boolean) => {
    if (isIconHijackSaving.value) return;
    const previousSettings = iconHijackSettings.value;
    iconHijackForm.enabled = nextValue;
    const result = await runSaveIconHijackSettings(
      () => SystemAPI.updateFnosPortIconHijackConfig({ enabled: nextValue }),
      {
        onSuccess: (data) => {
          applyIconHijackFromSettings(data);
          toast.success(t("admin.fnosSettings.iconHijackUpdated"));
        },
      },
    );
    if (!result && previousSettings) {
      applyIconHijackFromSettings(previousSettings);
    }
  };

  const saveNetworkTuning = async (
    patch: FnosNetworkTuningUpdatePayload,
    successKey: string,
  ) => {
    if (
      !networkTuningViewModel.isNetworkTuningAvailable.value ||
      isNetworkTuningSaving.value
    ) {
      if (!networkTuningViewModel.isNetworkTuningAvailable.value) {
        toast.error(t("admin.fnosSettings.unavailableTitle"), {
          description:
            networkTuningViewModel.networkTuningUnavailableText.value,
        });
      }
      return;
    }

    const previousStatus = networkTuningStatus.value;
    Object.assign(networkTuningForm, patch);
    const result = await runSaveNetworkTuningSettings(
      () => SystemAPI.updateFnosNetworkTuningConfig(patch),
      {
        onSuccess: (data) => {
          applyNetworkTuningFromStatus(data);
          toast.success(t(successKey));
        },
      },
    );
    if (!result && previousStatus) applyNetworkTuningFromStatus(previousStatus);
  };

  const toggleShareBypass = () => {
    void saveShareBypassEnabled(!form.enabled);
  };
  const toggleIconHijack = () => {
    void saveIconHijackEnabled(!iconHijackForm.enabled);
  };
  const openCertificateSync = () => {
    void router.push("/system/fnos-certificate-sync");
  };

  onMounted(() => {
    void fetchSettings();
  });

  return {
    ...networkTuningViewModel,
    canUseFnosCertificateSync,
    canUseFnosConnectWaf,
    canUseFnosNetworkTuning,
    certificateSyncDetails,
    form,
    iconHijackForm,
    isIconHijackSaving,
    isLoading,
    isNetworkTuningSaving,
    isRestrictedByRunMode,
    isSaving,
    isShareBypassMode,
    networkTuningForm,
    networkTuningStatus,
    openCertificateSync,
    saveIconHijackEnabled,
    saveNetworkTuning,
    saveShareBypassEnabled,
    showLoadingSkeleton,
    toggleIconHijack,
    toggleShareBypass,
  };
}
