import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import { normalizeGatewayPortalConfig } from "@/lib/gatewayPortal";
import {
  buildGatewayUnmatchedRoutePatch,
  normalizeGatewayUnmatchedRouteBehavior,
  normalizeGatewayUpstreamErrorDetail,
} from "@/lib/gatewayUnmatchedRoute";
import { useConfigStore } from "@/store/config";
import type { GatewaySettings } from "@/types";
import { useGatewaySubdomainEditorAvailability } from "./useGatewaySubdomainEditorAvailability";

type GatewaySettingsForm = Pick<
  GatewaySettings,
  | "auth_cache_ttl_seconds"
  | "auth_cache_unauthorized_ttl_seconds"
  | "portal"
  | "unmatched_route"
  | "crawler_blocker"
  | "reverse_proxy_throttle"
>;

export const useGatewaySettingsController = () => {
  const configStore = useConfigStore();
  const router = useRouter();
  const { t } = useI18n();
  const settings = ref<GatewaySettings | null>(null);
  const form = reactive<GatewaySettingsForm>({
    auth_cache_ttl_seconds: 1,
    auth_cache_unauthorized_ttl_seconds: 1,
    portal: normalizeGatewayPortalConfig(),
    unmatched_route: {
      behavior: "error_page",
      upstream_error_detail: "less",
    },
    crawler_blocker: {
      enabled: false,
      updated_at: null,
    },
    reverse_proxy_throttle: {
      enabled: true,
      requests_per_second: 100,
      burst: 200,
      block_seconds: 30,
    },
  });

  const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.gatewaySettings.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.gatewaySettings.loadFailedDescription"),
        ),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.gatewaySettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.gatewaySettings.saveSettingsFailedDescription"),
        ),
      });
    },
  });
  const isGatewaySettingsBusy = computed(() => isSaving.value);

  const clampCacheTtl = (value: unknown) =>
    Math.max(0, Math.floor(Number(value) || 0));
  const clampPositiveInt = (value: unknown, fallback = 1) =>
    Math.max(1, Math.floor(Number(value) || fallback));

  const isDirty = computed(() => {
    if (!settings.value) return false;
    return (
      settings.value.auth_cache_ttl_seconds !==
        Number(form.auth_cache_ttl_seconds) ||
      settings.value.auth_cache_unauthorized_ttl_seconds !==
        Number(form.auth_cache_unauthorized_ttl_seconds) ||
      settings.value.reverse_proxy_throttle.enabled !==
        form.reverse_proxy_throttle.enabled ||
      settings.value.reverse_proxy_throttle.requests_per_second !==
        Number(form.reverse_proxy_throttle.requests_per_second) ||
      settings.value.reverse_proxy_throttle.burst !==
        Number(form.reverse_proxy_throttle.burst) ||
      settings.value.reverse_proxy_throttle.block_seconds !==
        Number(form.reverse_proxy_throttle.block_seconds) ||
      settings.value.crawler_blocker.enabled !== form.crawler_blocker.enabled ||
      settings.value.unmatched_route.behavior !==
        form.unmatched_route.behavior ||
      settings.value.unmatched_route.upstream_error_detail !==
        form.unmatched_route.upstream_error_detail
    );
  });

  const authCacheHint = computed(() =>
    Number(form.auth_cache_ttl_seconds) === 0
      ? t("admin.gatewaySettings.authCacheDisabled")
      : t("admin.gatewaySettings.authCacheEnabled", {
          seconds: clampCacheTtl(form.auth_cache_ttl_seconds),
        }),
  );
  const authCacheFailHint = computed(() =>
    Number(form.auth_cache_unauthorized_ttl_seconds) === 0
      ? t("admin.gatewaySettings.authCacheFailDisabled")
      : t("admin.gatewaySettings.authCacheFailEnabled", {
          seconds: clampCacheTtl(form.auth_cache_unauthorized_ttl_seconds),
        }),
  );

  const visibilitySummary = computed(() => settings.value?.visibility ?? null);
  const proxyProtocolSummary = computed(
    () => settings.value?.proxy_protocol ?? null,
  );
  const portalSummary = computed(() => settings.value?.portal ?? null);
  const portalEnabledSummary = computed(() =>
    portalSummary.value?.enabled !== false
      ? t("admin.gatewaySettings.enabled")
      : t("admin.gatewaySettings.disabled"),
  );
  const portalDisplaySummary = computed(() =>
    portalSummary.value?.display_style === "domain"
      ? t("admin.gatewaySettings.portalDisplayDomain")
      : t("admin.gatewaySettings.portalDisplayTitle"),
  );
  const portalIconSummary = computed(() =>
    portalSummary.value?.show_app_icon !== false
      ? t("admin.gatewaySettings.enabled")
      : t("admin.gatewaySettings.disabled"),
  );
  const portalVersionSummary = computed(() =>
    portalSummary.value?.version === "v2"
      ? t("admin.gatewaySettings.portalVersionV2")
      : t("admin.gatewaySettings.portalVersionV1"),
  );
  const {
    isProxyHeadersAvailable,
    proxyHeadersDisabledReason,
    isHostResponseAvailable,
    hostResponseDisabledReason,
    isLocationsAvailable,
    locationsDisabledReason,
  } = useGatewaySubdomainEditorAvailability();

  const openVisibilityEditor = () =>
    void router.push("/system/gateway-visibility");
  const openPortalEditor = () => void router.push("/system/gateway-portal");
  const openProxyHeadersEditor = () => {
    if (isProxyHeadersAvailable.value) {
      void router.push("/system/gateway-proxy-headers");
    }
  };
  const openProxyProtocolEditor = () =>
    void router.push("/system/gateway-proxy-protocol");
  const openHostResponseEditor = () => {
    if (isHostResponseAvailable.value) {
      void router.push("/system/gateway-host-response");
    }
  };
  const openLocationsEditor = () => {
    if (isLocationsAvailable.value) {
      void router.push("/system/gateway-locations");
    }
  };

  const buildSettingsSnapshot = (data: GatewaySettings): GatewaySettings => ({
    ...data,
    portal: normalizeGatewayPortalConfig(data.portal),
    crawler_blocker: {
      enabled: data.crawler_blocker?.enabled === true,
      updated_at: data.crawler_blocker?.updated_at ?? null,
    },
    unmatched_route: {
      behavior: normalizeGatewayUnmatchedRouteBehavior(
        data.unmatched_route?.behavior,
      ),
      upstream_error_detail: normalizeGatewayUpstreamErrorDetail(
        data.unmatched_route?.upstream_error_detail,
      ),
    },
    reverse_proxy_throttle: { ...data.reverse_proxy_throttle },
  });

  const applyFromSettings = (data: GatewaySettings) => {
    const snapshot = buildSettingsSnapshot(data);
    settings.value = snapshot;
    form.auth_cache_ttl_seconds = data.auth_cache_ttl_seconds;
    form.auth_cache_unauthorized_ttl_seconds =
      data.auth_cache_unauthorized_ttl_seconds;
    form.portal.enabled = snapshot.portal.enabled;
    form.portal.display_style = snapshot.portal.display_style;
    form.portal.show_app_icon = snapshot.portal.show_app_icon;
    form.portal.icon_drag_mode = snapshot.portal.icon_drag_mode;
    form.portal.version = snapshot.portal.version;
    form.unmatched_route.behavior = snapshot.unmatched_route.behavior;
    form.unmatched_route.upstream_error_detail =
      snapshot.unmatched_route.upstream_error_detail;
    form.crawler_blocker.enabled = snapshot.crawler_blocker.enabled;
    form.crawler_blocker.updated_at = snapshot.crawler_blocker.updated_at;
    form.reverse_proxy_throttle.enabled = data.reverse_proxy_throttle.enabled;
    form.reverse_proxy_throttle.requests_per_second =
      data.reverse_proxy_throttle.requests_per_second;
    form.reverse_proxy_throttle.burst = data.reverse_proxy_throttle.burst;
    form.reverse_proxy_throttle.block_seconds =
      data.reverse_proxy_throttle.block_seconds;
  };

  const fetchSettings = async () => {
    await runLoadSettings(async () => {
      applyFromSettings(await ConfigAPI.getGatewaySettings());
    });
  };
  const resetForm = () => {
    if (settings.value) applyFromSettings(settings.value);
  };
  const saveSettings = async () => {
    await runSaveSettings(
      () =>
        ConfigAPI.updateGatewaySettings({
          auth_cache_ttl_seconds: clampCacheTtl(form.auth_cache_ttl_seconds),
          auth_cache_unauthorized_ttl_seconds: clampCacheTtl(
            form.auth_cache_unauthorized_ttl_seconds,
          ),
          reverse_proxy_throttle: {
            enabled: form.reverse_proxy_throttle.enabled,
            requests_per_second: clampPositiveInt(
              form.reverse_proxy_throttle.requests_per_second,
              100,
            ),
            burst: clampPositiveInt(form.reverse_proxy_throttle.burst, 200),
            block_seconds: clampPositiveInt(
              form.reverse_proxy_throttle.block_seconds,
              30,
            ),
          },
          crawler_blocker: {
            enabled: form.crawler_blocker.enabled,
          },
          ...buildGatewayUnmatchedRoutePatch(
            form.unmatched_route.behavior,
            form.unmatched_route.upstream_error_detail,
          ),
        }),
      {
        onSuccess: async (data) => {
          applyFromSettings(data);
          await configStore.loadConfig();
          toast.success(t("admin.gatewaySettings.settingsUpdated"));
        },
      },
    );
  };

  onMounted(fetchSettings);

  return {
    authCacheFailHint,
    authCacheHint,
    form,
    hostResponseDisabledReason,
    isDirty,
    isGatewaySettingsBusy,
    isHostResponseAvailable,
    isLoading,
    isLocationsAvailable,
    isProxyHeadersAvailable,
    isSaving,
    locationsDisabledReason,
    openHostResponseEditor,
    openLocationsEditor,
    openPortalEditor,
    openProxyHeadersEditor,
    openProxyProtocolEditor,
    openVisibilityEditor,
    portalDisplaySummary,
    portalEnabledSummary,
    portalIconSummary,
    portalSummary,
    portalVersionSummary,
    proxyHeadersDisabledReason,
    proxyProtocolSummary,
    resetForm,
    saveSettings,
    settings,
    showLoadingSkeleton,
    visibilitySummary,
  };
};
