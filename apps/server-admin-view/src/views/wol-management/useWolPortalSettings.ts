import { ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import { normalizeGatewayPortalConfig } from "@/lib/gatewayPortal";
import { useConfigStore } from "@/store/config";
import type { WolTranslate } from "./wol-management-types";

export const useWolPortalSettings = (t: WolTranslate) => {
  const configStore = useConfigStore();
  const settingsOpen = ref(false);
  const showWolInPortal = ref(true);
  const savingPortalSetting = ref(false);

  const openSettings = async () => {
    try {
      const data = await ConfigAPI.getGatewaySettings();
      showWolInPortal.value = normalizeGatewayPortalConfig(
        data.portal,
      ).show_wol;
      settingsOpen.value = true;
    } catch (error) {
      toast.error(t("admin.wol.portal.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.portal.loadFailed"),
        ),
      });
    }
  };

  const savePortalSetting = async () => {
    savingPortalSetting.value = true;
    try {
      const data = await ConfigAPI.updateGatewaySettings({
        portal: { show_wol: showWolInPortal.value },
      });
      showWolInPortal.value = normalizeGatewayPortalConfig(
        data.portal,
      ).show_wol;
      await configStore.loadConfig();
      settingsOpen.value = false;
      toast.success(t("admin.wol.portal.saved"));
    } catch (error) {
      toast.error(t("admin.wol.portal.saveFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.wol.portal.saveFailed"),
        ),
      });
    } finally {
      savingPortalSetting.value = false;
    }
  };

  return {
    openSettings,
    savePortalSetting,
    savingPortalSetting,
    settingsOpen,
    showWolInPortal,
  };
};
