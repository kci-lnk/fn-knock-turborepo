import { computed, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import type { FnosNetworkTuningStatus } from "@/types";

const displaySysctlValue = (value: string | null | undefined) =>
  value?.trim() || "--";

const displayList = (values: string[] | null | undefined) =>
  values && values.length > 0 ? values.join(" ") : "--";

export const useFnosNetworkTuningViewModel = (
  status: Ref<FnosNetworkTuningStatus | null>,
) => {
  const { t } = useI18n();
  const isNetworkTuningAvailable = computed(
    () => status.value?.available === true,
  );
  const isBbrSupported = computed(() => status.value?.bbr.supported === true);
  const isMtuProbingSupported = computed(
    () => status.value?.mtu_probing.supported === true,
  );
  const networkTuningUnavailableText = computed(
    () =>
      status.value?.blocked_reason ||
      t("admin.fnosSettings.networkTuningUnavailable"),
  );
  const desiredStateText = (enabled: boolean | null | undefined) =>
    t(
      enabled
        ? "admin.fnosSettings.desiredEnabled"
        : "admin.fnosSettings.desiredDisabled",
    );
  const bbrDesiredDescription = computed(() =>
    t("admin.fnosSettings.desiredState", {
      state: desiredStateText(status.value?.config.bbr_enabled),
    }),
  );
  const bbrCurrentDescription = computed(() =>
    t("admin.fnosSettings.bbrCurrent", {
      congestion: displaySysctlValue(
        status.value?.bbr.current_congestion_control,
      ),
      qdisc: displaySysctlValue(status.value?.bbr.current_default_qdisc),
      available: displayList(status.value?.bbr.available_congestion_control),
    }),
  );
  const bbrSupportDescription = computed(() => {
    if (!status.value) return "";
    return status.value.bbr.supported
      ? t("admin.fnosSettings.bbrSupported")
      : t("admin.fnosSettings.bbrUnsupported");
  });
  const bbrStateMismatchDescription = computed(() => {
    if (!status.value) return "";
    if (status.value.config.bbr_enabled && !status.value.bbr.active) {
      return t("admin.fnosSettings.bbrRuntimeInactiveAfterEnable");
    }
    if (!status.value.config.bbr_enabled && status.value.bbr.active) {
      return t("admin.fnosSettings.bbrRuntimeStillActiveAfterDisable");
    }
    return "";
  });
  const mtuDesiredDescription = computed(() =>
    t("admin.fnosSettings.desiredState", {
      state: desiredStateText(status.value?.config.mtu_probing_enabled),
    }),
  );
  const mtuCurrentDescription = computed(() =>
    t("admin.fnosSettings.mtuCurrent", {
      value: displaySysctlValue(status.value?.mtu_probing.current_value),
    }),
  );
  const mtuStateMismatchDescription = computed(() => {
    if (!status.value) return "";
    if (
      status.value.config.mtu_probing_enabled &&
      !status.value.mtu_probing.active
    ) {
      return t("admin.fnosSettings.mtuRuntimeInactiveAfterEnable");
    }
    if (
      !status.value.config.mtu_probing_enabled &&
      status.value.mtu_probing.active
    ) {
      return t("admin.fnosSettings.mtuRuntimeStillActiveAfterDisable", {
        value: displaySysctlValue(status.value.mtu_probing.current_value),
      });
    }
    return "";
  });

  return {
    bbrCurrentDescription,
    bbrDesiredDescription,
    bbrStateMismatchDescription,
    bbrSupportDescription,
    isBbrSupported,
    isMtuProbingSupported,
    isNetworkTuningAvailable,
    mtuCurrentDescription,
    mtuDesiredDescription,
    mtuStateMismatchDescription,
    networkTuningUnavailableText,
  };
};
