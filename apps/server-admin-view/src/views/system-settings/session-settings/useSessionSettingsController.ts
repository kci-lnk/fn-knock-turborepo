import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ConfigAPI } from "@/lib/api/config";
import type {
  AuthCredentialSettings,
  PostLoginIpGrantMode,
} from "../../../types";
import { useConfigStore } from "../../../store/config";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import {
  durationUnits,
  ipGrantDurationUnits,
  mobilityWindowDurationUnits,
  splitDuration,
  toDurationSeconds,
  type SessionDurationField,
} from "./sessionDurationModel";
import { useSessionCookieScope } from "./useSessionCookieScope";

type SessionSettingsForm = {
  session: SessionDurationField;
  rememberMe: SessionDurationField;
  postLoginIpGrantMode: PostLoginIpGrantMode;
  customGrant: SessionDurationField;
  sessionIpMobilityEnabled: boolean;
  sessionIpMobilityWindow: SessionDurationField;
};

export function useSessionSettingsController() {
  const configStore = useConfigStore();
  const { t } = useI18n();
  const settings = ref<AuthCredentialSettings | null>(null);
  const form = reactive<SessionSettingsForm>({
    session: { value: 24, unit: "hour" },
    rememberMe: { value: 1, unit: "year" },
    postLoginIpGrantMode: "follow_session",
    customGrant: { value: 1, unit: "hour" },
    sessionIpMobilityEnabled: false,
    sessionIpMobilityWindow: { value: 20, unit: "minute" },
  });

  const postLoginIpGrantModeOptions = computed(() => [
    {
      value: "follow_session" as const,
      title: t("admin.sessionSettings.grantModes.followSession.title"),
      description: t(
        "admin.sessionSettings.grantModes.followSession.description",
      ),
    },
    {
      value: "disabled" as const,
      title: t("admin.sessionSettings.grantModes.disabled.title"),
      description: t("admin.sessionSettings.grantModes.disabled.description"),
    },
    {
      value: "custom" as const,
      title: t("admin.sessionSettings.grantModes.custom.title"),
      description: t("admin.sessionSettings.grantModes.custom.description"),
    },
  ]);

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessionSettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessionSettings.loadDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessionSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessionSettings.saveDescription"),
        ),
      });
    },
  });

  const formatDuration = (seconds: number, units = durationUnits): string => {
    const normalized = splitDuration(seconds, units);
    const label =
      units.find((item) => item.value === normalized.unit)?.labelKey || "";
    const unitLabel = label ? t(label) : normalized.unit;
    return `${normalized.value} ${unitLabel}`;
  };

  const cookieScope = useSessionCookieScope();
  const sessionTtlSeconds = computed(() => toDurationSeconds(form.session));
  const rememberMeTtlSeconds = computed(() =>
    toDurationSeconds(form.rememberMe),
  );
  const customGrantTtlSeconds = computed(() =>
    toDurationSeconds(form.customGrant),
  );
  const sessionIpMobilityWindowSeconds = computed(() =>
    toDurationSeconds(form.sessionIpMobilityWindow),
  );

  const isDirty = computed(() => {
    if (!settings.value) return false;
    const storedGrantTtl =
      settings.value.post_login_ip_grant_ttl_seconds ?? 3_600;
    const shouldCompareCustomGrantTtl =
      settings.value.post_login_ip_grant_mode === "custom" ||
      form.postLoginIpGrantMode === "custom";
    return (
      settings.value.session_ttl_seconds !== sessionTtlSeconds.value ||
      settings.value.remember_me_ttl_seconds !== rememberMeTtlSeconds.value ||
      settings.value.post_login_ip_grant_mode !== form.postLoginIpGrantMode ||
      settings.value.session_ip_mobility_enabled !==
        form.sessionIpMobilityEnabled ||
      settings.value.session_ip_mobility_window_seconds !==
        sessionIpMobilityWindowSeconds.value ||
      (shouldCompareCustomGrantTtl &&
        storedGrantTtl !== customGrantTtlSeconds.value)
    );
  });

  const grantModeSummary = computed(() => {
    switch (form.postLoginIpGrantMode) {
      case "follow_session":
        return t("admin.sessionSettings.grantSummary.followSession");
      case "disabled":
        return t("admin.sessionSettings.grantSummary.disabled");
      case "custom":
        return t("admin.sessionSettings.grantSummary.custom", {
          duration: formatDuration(
            customGrantTtlSeconds.value,
            ipGrantDurationUnits,
          ),
        });
      default:
        return "";
    }
  });

  const sessionIpMobilitySummary = computed(() => {
    if (!form.sessionIpMobilityEnabled) {
      return t("admin.sessionSettings.mobilitySummary.disabled");
    }
    return t("admin.sessionSettings.mobilitySummary.enabled", {
      duration: formatDuration(
        sessionIpMobilityWindowSeconds.value,
        mobilityWindowDurationUnits,
      ),
    });
  });

  const applyFromSettings = (data: AuthCredentialSettings) => {
    settings.value = data;
    Object.assign(form.session, splitDuration(data.session_ttl_seconds));
    Object.assign(form.rememberMe, splitDuration(data.remember_me_ttl_seconds));
    form.postLoginIpGrantMode = data.post_login_ip_grant_mode;
    Object.assign(
      form.customGrant,
      splitDuration(
        data.post_login_ip_grant_ttl_seconds ?? 3_600,
        ipGrantDurationUnits,
      ),
    );
    form.sessionIpMobilityEnabled = data.session_ip_mobility_enabled === true;
    Object.assign(
      form.sessionIpMobilityWindow,
      splitDuration(
        data.session_ip_mobility_window_seconds ?? 20 * 60,
        mobilityWindowDurationUnits,
      ),
    );
  };

  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      applyFromSettings(await ConfigAPI.getAuthCredentialSettings());
    });
  };
  const resetForm = () => {
    if (settings.value) applyFromSettings(settings.value);
  };

  const showValidationError = (descriptionKey: string) => {
    toast.error(t("admin.sessionSettings.invalidSettings"), {
      description: t(descriptionKey),
    });
  };

  const saveSettings = async () => {
    const nextSessionTtl = sessionTtlSeconds.value;
    const nextRememberMeTtl = rememberMeTtlSeconds.value;
    const nextCustomGrantTtl = customGrantTtlSeconds.value;
    const nextMobilityWindowSeconds = sessionIpMobilityWindowSeconds.value;

    if (nextSessionTtl < 60 || nextRememberMeTtl < 60) {
      toast.error(t("admin.sessionSettings.tooShort"), {
        description: t("admin.sessionSettings.sessionTooShortDescription"),
      });
      return;
    }
    if (nextRememberMeTtl < nextSessionTtl) {
      showValidationError(
        "admin.sessionSettings.rememberMeShorterDescription",
      );
      return;
    }
    if (form.postLoginIpGrantMode === "custom" && nextCustomGrantTtl < 60) {
      showValidationError(
        "admin.sessionSettings.customGrantTooShortDescription",
      );
      return;
    }
    if (
      form.sessionIpMobilityEnabled &&
      (nextMobilityWindowSeconds < 60 ||
        nextMobilityWindowSeconds > 24 * 3_600)
    ) {
      showValidationError(
        "admin.sessionSettings.mobilityWindowInvalidDescription",
      );
      return;
    }

    await runSaveSettings(
      () =>
        ConfigAPI.updateAuthCredentialSettings({
          session_ttl_seconds: nextSessionTtl,
          remember_me_ttl_seconds: nextRememberMeTtl,
          post_login_ip_grant_mode: form.postLoginIpGrantMode,
          post_login_ip_grant_ttl_seconds:
            form.postLoginIpGrantMode === "custom"
              ? nextCustomGrantTtl
              : null,
          session_ip_mobility_enabled: form.sessionIpMobilityEnabled,
          session_ip_mobility_window_seconds: nextMobilityWindowSeconds,
        }),
      {
        onSuccess: async (data) => {
          applyFromSettings(data);
          await configStore.loadConfig();
          toast.success(t("admin.sessionSettings.updated"));
        },
      },
    );
  };

  onMounted(() => {
    void fetchSettings();
  });

  return {
    ...cookieScope,
    customGrantTtlSeconds,
    durationUnits,
    form,
    formatDuration,
    grantModeSummary,
    ipGrantDurationUnits,
    isDirty,
    isLoading,
    isSaving,
    mobilityWindowDurationUnits,
    postLoginIpGrantModeOptions,
    rememberMeTtlSeconds,
    resetForm,
    saveSettings,
    sessionIpMobilitySummary,
    sessionIpMobilityWindowSeconds,
    sessionTtlSeconds,
    showLoadingSkeleton,
  };
}

export type SessionSettingsController = ReturnType<
  typeof useSessionSettingsController
>;
