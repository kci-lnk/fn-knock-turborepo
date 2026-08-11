import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { IpLocationSettingsAPI, type IpLocationApiConfig } from "@/lib/api";
import {
  buildIpLocationSettingsPayload,
  DEFAULT_CUSTOM_CIDR_URL,
  DEFAULT_CUSTOM_IP_LOOKUP_URL,
  isHttpUrl,
  normalizeIpLocationBaseUrl,
  normalizeIpLocationSettings,
} from "./ipLocationSettingsModel";

export const useIpLocationSettings = () => {
  const { t } = useI18n();
  const settings = ref<IpLocationApiConfig | null>(null);
  const form = reactive<
    Pick<IpLocationApiConfig, "ip_lookup_mode" | "cidr_mode">
  >({ ip_lookup_mode: "online", cidr_mode: "online" });
  const ipLookupUrlInput = ref("");
  const cidrUrlInput = ref("");

  const applyDefaultCustomUrls = () => {
    if (
      form.ip_lookup_mode === "custom" &&
      !normalizeIpLocationBaseUrl(ipLookupUrlInput.value)
    ) {
      ipLookupUrlInput.value = DEFAULT_CUSTOM_IP_LOOKUP_URL;
    }
    if (
      form.cidr_mode === "custom" &&
      !normalizeIpLocationBaseUrl(cidrUrlInput.value)
    ) {
      cidrUrlInput.value = DEFAULT_CUSTOM_CIDR_URL;
    }
  };
  const currentPayload = computed(() =>
    buildIpLocationSettingsPayload({
      cidrMode: form.cidr_mode,
      cidrUrl: cidrUrlInput.value,
      ipLookupMode: form.ip_lookup_mode,
      ipLookupUrl: ipLookupUrlInput.value,
    }),
  );

  const validateCustomUrls = (payload: IpLocationApiConfig) => {
    if (payload.ip_lookup_mode === "custom") {
      if (!payload.ip_lookup_url) {
        toast.error(t("admin.ipLocationSettings.ipLookupUrlRequired"));
        return false;
      }
      if (!isHttpUrl(payload.ip_lookup_url)) {
        toast.error(t("admin.ipLocationSettings.ipLookupUrlInvalid"), {
          description: t("admin.ipLocationSettings.httpUrlRequired"),
        });
        return false;
      }
    }
    if (payload.cidr_mode === "custom") {
      if (!payload.cidr_url) {
        toast.error(t("admin.ipLocationSettings.cidrUrlRequired"));
        return false;
      }
      if (!isHttpUrl(payload.cidr_url)) {
        toast.error(t("admin.ipLocationSettings.cidrUrlInvalid"), {
          description: t("admin.ipLocationSettings.httpUrlRequired"),
        });
        return false;
      }
    }
    return true;
  };

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ipLocationSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ipLocationSettings.loadFailedDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ipLocationSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ipLocationSettings.saveFailedDescription"),
        ),
      });
    },
  });
  const { isPending: isTestingIpLookup, run: runTestIpLookup } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.ipLocationSettings.connectionFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.ipLocationSettings.ipLookupUnavailable"),
          ),
        });
      },
    });
  const { isPending: isTestingCidr, run: runTestCidr } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ipLocationSettings.connectionFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ipLocationSettings.cidrUnavailable"),
        ),
      });
    },
  });

  const isDirty = computed(() => {
    if (!settings.value) return false;
    const payload = currentPayload.value;
    return (
      settings.value.ip_lookup_mode !== payload.ip_lookup_mode ||
      settings.value.ip_lookup_url !== payload.ip_lookup_url ||
      settings.value.cidr_mode !== payload.cidr_mode ||
      settings.value.cidr_url !== payload.cidr_url
    );
  });

  const applyFromSettings = (data: IpLocationApiConfig) => {
    const normalized = normalizeIpLocationSettings(data);
    settings.value = normalized;
    form.ip_lookup_mode = normalized.ip_lookup_mode;
    form.cidr_mode = normalized.cidr_mode;
    ipLookupUrlInput.value =
      normalized.ip_lookup_mode === "custom" ? normalized.ip_lookup_url : "";
    cidrUrlInput.value =
      normalized.cidr_mode === "custom" ? normalized.cidr_url : "";
    applyDefaultCustomUrls();
  };
  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      applyFromSettings(await IpLocationSettingsAPI.getSettings());
    });
  };
  const resetForm = () => {
    if (settings.value) applyFromSettings(settings.value);
  };

  const validateTestUrl = (value: string, kind: "ipLookup" | "cidr") => {
    const url = normalizeIpLocationBaseUrl(value);
    if (!url) {
      toast.error(
        t(
          kind === "ipLookup"
            ? "admin.ipLocationSettings.ipLookupUrlInputRequired"
            : "admin.ipLocationSettings.cidrUrlInputRequired",
        ),
      );
      return "";
    }
    if (!isHttpUrl(url)) {
      toast.error(
        t(
          kind === "ipLookup"
            ? "admin.ipLocationSettings.ipLookupUrlInvalid"
            : "admin.ipLocationSettings.cidrUrlInvalid",
        ),
        { description: t("admin.ipLocationSettings.httpUrlRequired") },
      );
      return "";
    }
    return url;
  };

  const testIpLookupService = async () => {
    const url = validateTestUrl(ipLookupUrlInput.value, "ipLookup");
    if (!url) return;
    await runTestIpLookup(async () => {
      const result = await IpLocationSettingsAPI.testIpLookup(url);
      if (result.success) {
        toast.success(t("admin.ipLocationSettings.connectionSuccess"), {
          description: t("admin.ipLocationSettings.ipLookupHealthy"),
        });
      } else {
        toast.error(t("admin.ipLocationSettings.connectionFailed"), {
          description:
            result.message ||
            result.msg ||
            t("admin.ipLocationSettings.ipLookupUnavailable"),
        });
      }
    });
  };
  const testCidrService = async () => {
    const url = validateTestUrl(cidrUrlInput.value, "cidr");
    if (!url) return;
    await runTestCidr(async () => {
      const result = await IpLocationSettingsAPI.testCidr(url);
      if (!result.success) {
        toast.error(t("admin.ipLocationSettings.connectionFailed"), {
          description:
            result.message || t("admin.ipLocationSettings.cidrUnavailable"),
        });
        return;
      }
      if (result.capabilities?.operatorFiltering.supported === false) {
        toast.warning(t("admin.ipLocationSettings.cidrUpgradeRequiredTitle"), {
          description: t("admin.ipLocationSettings.cidrUpgradeRequired", {
            version:
              result.capabilities.operatorFiltering.minimumContainerVersion,
          }),
        });
      } else {
        toast.success(t("admin.ipLocationSettings.connectionSuccess"), {
          description: t("admin.ipLocationSettings.cidrHealthy"),
        });
      }
    });
  };
  const saveSettings = async () => {
    const payload = currentPayload.value;
    if (!validateCustomUrls(payload)) return;
    await runSaveSettings(() => IpLocationSettingsAPI.updateSettings(payload), {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success(t("admin.ipLocationSettings.settingsUpdated"));
      },
    });
  };

  watch(
    () => [form.ip_lookup_mode, form.cidr_mode] as const,
    applyDefaultCustomUrls,
  );
  onMounted(() => {
    void fetchSettings();
  });

  return {
    cidrDockerUrl: "https://hub.docker.com/r/kcilnk/go-cidr-api",
    cidrUrlInput,
    form,
    ipLookupDockerUrl: "https://hub.docker.com/r/kcilnk/go-ipaddress-api",
    ipLookupUrlInput,
    isDirty,
    isLoading,
    isSaving,
    isTestingCidr,
    isTestingIpLookup,
    resetForm,
    saveSettings,
    showLoadingSkeleton,
    t,
    testCidrService,
    testIpLookupService,
  };
};
