import { computed, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useConfigStore } from "@/store/config";
import type {
  SmartConnectConfig,
  SmartConnectDetails,
  SmartConnectLocalIpOption,
} from "@/types";
import { hasUnsavedSmartConnectDraft } from "./smartConnectModel";

export const useSmartConnectViewModel = (options: {
  details: Ref<SmartConnectDetails | null>;
  form: SmartConnectConfig;
}) => {
  const { t } = useI18n();
  const configStore = useConfigStore();
  const isDirty = computed(() =>
    hasUnsavedSmartConnectDraft(options.details.value, options.form),
  );
  const capabilityBlockedReason = computed(() => {
    if (configStore.canUseSmartConnect) return "";
    return configStore.isDockerDeployment
      ? t("admin.smartConnectSettings.dockerUnsupported")
      : t("admin.smartConnectSettings.environmentUnsupported");
  });
  const isSmartConnectAvailable = computed(
    () => options.details.value?.availability.available === true,
  );
  const showDnsmasqCard = computed(() => options.form.enabled);
  const isDnsmasqReady = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    return Boolean(
      dnsmasq &&
      dnsmasq.install_state.status !== "installing" &&
      dnsmasq.install_state.status !== "error" &&
      dnsmasq.installed &&
      dnsmasq.service_active &&
      dnsmasq.initialized,
    );
  });
  const showDnsmasqSetupCard = computed(
    () => showDnsmasqCard.value && !isDnsmasqReady.value,
  );
  const showAdvancedCards = computed(
    () => options.form.enabled && isDnsmasqReady.value,
  );
  const dnsmasqSummaryText = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    if (!dnsmasq) return "";
    return [
      dnsmasq.service_active
        ? t("admin.smartConnectSettings.serviceRunning")
        : t("admin.smartConnectSettings.serviceStopped"),
      t("admin.smartConnectSettings.managedRules", {
        count: dnsmasq.runtime.managed_rule_count,
      }),
    ].join(" · ");
  });
  const dnsmasqProgress = computed(() => {
    const value = Number(
      options.details.value?.dnsmasq.install_state.progress ?? 0,
    );
    return Number.isFinite(value) ? Math.max(0, Math.min(100, value)) : 0;
  });
  const dnsmasqStatusLabel = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    if (!dnsmasq) return t("admin.smartConnectSettings.loading");
    if (dnsmasq.install_state.status === "installing") {
      return t("admin.smartConnectSettings.installing");
    }
    if (dnsmasq.install_state.status === "error") {
      return t("admin.smartConnectSettings.abnormal");
    }
    if (!dnsmasq.installed) {
      return t("admin.smartConnectSettings.notInstalled");
    }
    if (!dnsmasq.service_active) {
      return t("admin.smartConnectSettings.notRunning");
    }
    if (!dnsmasq.initialized) {
      return t("admin.smartConnectSettings.pendingInitialization");
    }
    return t("admin.smartConnectSettings.ready");
  });
  const dnsmasqStatusVariant = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    if (!dnsmasq || !dnsmasq.installed) return "outline";
    if (dnsmasq.install_state.status === "error" || !dnsmasq.service_active) {
      return "destructive";
    }
    if (dnsmasq.install_state.status === "installing" || !dnsmasq.initialized) {
      return "secondary";
    }
    return "default";
  });
  const dnsmasqNeedsInitialization = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    return Boolean(
      dnsmasq?.installed && (!dnsmasq.service_active || !dnsmasq.initialized),
    );
  });
  const showDnsmasqAction = computed(() => {
    const dnsmasq = options.details.value?.dnsmasq;
    return Boolean(
      dnsmasq &&
      (dnsmasq.install_state.status === "installing" ||
        dnsmasq.install_state.status === "error" ||
        !dnsmasq.installed ||
        dnsmasqNeedsInitialization.value),
    );
  });
  const resolvedIpOptions = computed<SmartConnectLocalIpOption[]>(() => {
    const currentOptions = options.details.value?.local_ip_options ?? [];
    if (
      !options.form.selected_ipv4 ||
      currentOptions.some((item) => item.value === options.form.selected_ipv4)
    ) {
      return currentOptions;
    }
    return [
      ...currentOptions,
      {
        label: t("admin.smartConnectSettings.currentConfiguredIpUnavailable", {
          ip: options.form.selected_ipv4,
        }),
        value: options.form.selected_ipv4,
        interface: "manual",
      },
    ];
  });
  const saveBlockedReason = computed(() => {
    if (!configStore.canUseSmartConnect) return capabilityBlockedReason.value;
    if (!options.form.enabled) return "";
    if (!isSmartConnectAvailable.value) {
      return (
        options.details.value?.availability.reason ||
        t("admin.smartConnectSettings.currentModeUnavailable")
      );
    }
    if (!options.details.value?.dnsmasq.initialized) {
      return t("admin.smartConnectSettings.initializeDnsmasqFirst");
    }
    if (!options.form.selected_ipv4) {
      return t("admin.smartConnectSettings.selectLocalIpFirst");
    }
    if ((options.details.value?.domains.length ?? 0) === 0) {
      return t("admin.smartConnectSettings.noDomainsToSync");
    }
    return "";
  });

  return {
    capabilityBlockedReason,
    dnsmasqActionLabel: computed(() =>
      t("admin.smartConnectSettings.initialize"),
    ),
    dnsmasqProgress,
    dnsmasqStatusLabel,
    dnsmasqStatusVariant,
    dnsmasqSummaryText,
    isDirty,
    isSmartConnectAvailable,
    resolvedIpOptions,
    saveBlockedReason,
    showAdvancedCards,
    showDnsmasqAction,
    showDnsmasqCard,
    showDnsmasqSetupCard,
  };
};
