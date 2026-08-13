import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { ScannerAPI, type ScannerSettings } from "@/lib/api/security";
import type { GatewayVisibilitySelection } from "@/types";
import { getCidrRegionSelectionKey } from "@/types/cidr";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { parseCidrTextarea } from "@admin-shared/utils/cidr";
import { toast } from "@admin-shared/utils/toast";

export type ScannerFirewallForm = {
  enabled: boolean;
  commonLocationExemptEnabled: boolean;
  windowMinutes: number;
  threshold: number;
  blacklistTtlDays: number;
  cidrExemptionRegions: GatewayVisibilitySelection[];
  cidrExemptionsText: string;
};

const BASE_WINDOW_MINUTES = 5;

export const useScannerFirewallSettings = () => {
  const { t } = useI18n();
  const router = useRouter();
  const settings = ref<ScannerSettings | null>(null);
  const form = reactive<ScannerFirewallForm>({
    enabled: true,
    commonLocationExemptEnabled: false,
    windowMinutes: BASE_WINDOW_MINUTES,
    threshold: 3,
    blacklistTtlDays: 90,
    cidrExemptionRegions: [],
    cidrExemptionsText: "",
  });

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.scannerFirewallSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.scannerFirewallSettings.loadDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.scannerFirewallSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.scannerFirewallSettings.saveDescription"),
        ),
      });
    },
  });

  const derivedWindowMinutes = computed(() =>
    Math.max(BASE_WINDOW_MINUTES, Number(form.windowMinutes) || 0),
  );
  const cidrExemptionsState = computed(() =>
    parseCidrTextarea(form.cidrExemptionsText),
  );
  const invalidCidrExemptions = computed(
    () => cidrExemptionsState.value.invalid,
  );
  const regionInputsDisabled = computed(
    () => isSaving.value || !form.enabled,
  );
  const isDirty = computed(() => {
    if (!settings.value) return false;
    const savedRegionKeys = (settings.value.cidrExemptionRegions ?? []).map(
      getCidrRegionSelectionKey,
    );
    const formRegionKeys = form.cidrExemptionRegions.map(
      getCidrRegionSelectionKey,
    );
    return (
      settings.value.enabled !== form.enabled ||
      settings.value.commonLocationExemptEnabled !==
        form.commonLocationExemptEnabled ||
      settings.value.windowMinutes !== Number(form.windowMinutes) ||
      settings.value.threshold !== Number(form.threshold) ||
      Math.ceil(settings.value.blacklistTtlSeconds / 86400) !==
        Number(form.blacklistTtlDays) ||
      JSON.stringify(savedRegionKeys) !== JSON.stringify(formRegionKeys) ||
      JSON.stringify(settings.value.cidrExemptions ?? []) !==
        JSON.stringify(cidrExemptionsState.value.cidrs)
    );
  });
  const saveBlockedReason = computed(() =>
    invalidCidrExemptions.value.length > 0
      ? t("admin.scannerFirewallSettings.fixCidrExemptions")
      : "",
  );

  const applyFromSettings = (data: ScannerSettings) => {
    settings.value = data;
    Object.assign(form, {
      enabled: data.enabled,
      commonLocationExemptEnabled:
        data.commonLocationExemptEnabled === true,
      windowMinutes: data.windowMinutes,
      threshold: data.threshold,
      blacklistTtlDays: Math.max(
        1,
        Math.ceil(data.blacklistTtlSeconds / 86400),
      ),
      cidrExemptionRegions: (data.cidrExemptionRegions ?? []).map((item) => ({
        ...item,
      })),
      cidrExemptionsText: (data.cidrExemptions ?? []).join("\n"),
    });
  };
  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      applyFromSettings(await ScannerAPI.getSettings());
    });
  };
  const resetForm = () => {
    if (settings.value) applyFromSettings(settings.value);
  };
  const saveSettings = async () => {
    if (invalidCidrExemptions.value.length > 0) {
      toast.error(t("admin.scannerFirewallSettings.cidrValidationFailed"), {
        description: t(
          "admin.scannerFirewallSettings.cidrExemptionsInvalid",
          { items: invalidCidrExemptions.value.join("、") },
        ),
      });
      return;
    }
    await runSaveSettings(
      () =>
        ScannerAPI.saveSettings({
          enabled: form.enabled,
          commonLocationExemptEnabled: form.commonLocationExemptEnabled,
          windowMinutes: Math.max(1, Number(form.windowMinutes) || 1),
          threshold: Math.max(1, Number(form.threshold) || 1),
          blacklistTtlSeconds: Math.max(
            60,
            Math.floor((Number(form.blacklistTtlDays) || 1) * 86400),
          ),
          cidrExemptionRegions: form.cidrExemptionRegions.map((item) => ({
            province: item.province,
            query_city: item.query_city,
            operator: item.operator,
          })),
          cidrExemptions: cidrExemptionsState.value.cidrs,
        }),
      {
        onSuccess: (data) => {
          applyFromSettings(data);
          toast.success(t("admin.scannerFirewallSettings.updated"));
        },
      },
    );
  };
  const goToBlacklist = () => {
    void router.push({ path: "/sessions", query: { tab: "ip-blacklist" } });
  };

  onMounted(() => void fetchSettings());

  return reactive({
    baseWindowMinutes: BASE_WINDOW_MINUTES,
    cidrExemptionsState,
    derivedWindowMinutes,
    form,
    goToBlacklist,
    invalidCidrExemptions,
    isDirty,
    isLoading,
    isSaving,
    regionInputsDisabled,
    resetForm,
    saveBlockedReason,
    saveSettings,
    showLoadingSkeleton,
  });
};

export type ScannerFirewallSettingsModel = ReturnType<
  typeof useScannerFirewallSettings
>;
