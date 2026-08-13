import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { ConfigAPI } from "@/lib/api/config";
import {
  buildGatewayPortalVersionPatch,
  normalizeGatewayPortalConfig,
} from "@/lib/gatewayPortal";
import { useConfigStore } from "@/store/config";
import type {
  GatewayPortalConfig,
  GatewayPortalDisplayStyle,
  GatewayPortalIconDragMode,
  GatewayPortalVersion,
  GatewaySettings,
} from "@/types";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";

export const useGatewayPortalSettings = () => {
  const { t } = useI18n();
  const configStore = useConfigStore();
  const settings = ref<GatewayPortalConfig | null>(null);
  const loadError = ref("");
  const form = reactive<GatewayPortalConfig>({
    enabled: true,
    display_style: "title",
    show_app_icon: true,
    show_wol: true,
    icon_drag_mode: "corners",
    version: "v1",
  });
  const wolFeatureEnabled = computed(
    () => configStore.config?.wol_feature?.enabled === true,
  );

  const applyPortal = (portal?: Partial<GatewayPortalConfig> | null) => {
    const normalized = normalizeGatewayPortalConfig(portal);
    settings.value = normalized;
    Object.assign(form, normalized);
  };
  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      loadError.value = extractErrorMessage(
        error,
        t("admin.gatewayPortalSettings.loadFailedDescription"),
      );
    },
  });
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.gatewayPortalSettings.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.gatewayPortalSettings.saveFailedDescription"),
        ),
      });
    },
  });

  const refreshConfigStore = async () => {
    try {
      await configStore.loadConfig();
    } catch (error) {
      console.error("[gateway-portal] failed to refresh config store:", error);
    }
  };
  const applySavedSettings = async (data: GatewaySettings | undefined) => {
    if (!data) return false;
    applyPortal(data.portal);
    await refreshConfigStore();
    toast.success(t("admin.gatewayPortalSettings.updated"));
    return true;
  };
  const loadSettings = async () => {
    await runLoad(async () => {
      loadError.value = "";
      const data = await ConfigAPI.getGatewaySettings();
      applyPortal(data.portal);
    });
  };

  const savePortalPatch = async (patch: Partial<GatewayPortalConfig>) => {
    if (isSaving.value) return;
    const previous = { ...form };
    Object.assign(form, patch);
    const data = await runSave(() =>
      ConfigAPI.updateGatewaySettings({ portal: patch }),
    );
    if (!(await applySavedSettings(data))) applyPortal(previous);
  };
  const saveEnabled = (enabled: boolean) => {
    if (form.enabled !== enabled) return savePortalPatch({ enabled });
  };
  const saveDisplayStyle = (displayStyle: GatewayPortalDisplayStyle) => {
    if (form.display_style !== displayStyle) {
      return savePortalPatch({ display_style: displayStyle });
    }
  };
  const saveIconDragMode = (iconDragMode: GatewayPortalIconDragMode) => {
    if (form.icon_drag_mode !== iconDragMode) {
      return savePortalPatch({ icon_drag_mode: iconDragMode });
    }
  };
  const saveShowAppIcon = (showAppIcon: boolean) => {
    if (form.show_app_icon !== showAppIcon) {
      return savePortalPatch({ show_app_icon: showAppIcon });
    }
  };
  const saveShowWOL = (showWol: boolean) => {
    if (form.show_wol !== showWol) {
      return savePortalPatch({ show_wol: showWol });
    }
  };
  const saveVersion = async (version: GatewayPortalVersion) => {
    if (isSaving.value || form.version === version) return;
    const previous = { ...form };
    form.version = version;
    const data = await runSave(() =>
      ConfigAPI.updateGatewaySettings(buildGatewayPortalVersionPatch(version)),
    );
    if (!(await applySavedSettings(data))) applyPortal(previous);
  };

  onMounted(() => void loadSettings());

  return reactive({
    form,
    isLoading,
    isSaving,
    loadError,
    saveDisplayStyle,
    saveEnabled,
    saveIconDragMode,
    saveShowAppIcon,
    saveShowWOL,
    saveVersion,
    wolFeatureEnabled,
  });
};

export type GatewayPortalSettingsModel = ReturnType<
  typeof useGatewayPortalSettings
>;
