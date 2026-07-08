import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { isAnySubdomainRoutingMode } from "../../lib/reverse-proxy-submode";
import { useConfigStore } from "../../store/config";

const runTypeLabelKeyMap = {
  0: "admin.gatewaySettings.runTypes.direct",
  1: "admin.gatewaySettings.runTypes.reverse",
  3: "admin.gatewaySettings.runTypes.subdomain",
} as const;

export const useGatewaySubdomainEditorAvailability = () => {
  const configStore = useConfigStore();
  const { t } = useI18n();

  const currentRunTypeLabel = computed(() => {
    const runType = configStore.config?.run_type;
    if (runType === 0 || runType === 1 || runType === 3) {
      return t(runTypeLabelKeyMap[runType]);
    }
    return t("admin.gatewaySettings.runTypes.current");
  });

  const isEditorAvailable = computed(() =>
    isAnySubdomainRoutingMode(configStore.config),
  );
  const disabledReason = computed(() => {
    if (isEditorAvailable.value) return "";
    return t("admin.gatewaySettings.subdomainOnlyReason", {
      mode: currentRunTypeLabel.value,
    });
  });

  return {
    isProxyHeadersAvailable: isEditorAvailable,
    proxyHeadersDisabledReason: disabledReason,
    isHostResponseAvailable: isEditorAvailable,
    hostResponseDisabledReason: disabledReason,
    isLocationsAvailable: isEditorAvailable,
    locationsDisabledReason: disabledReason,
  };
};
