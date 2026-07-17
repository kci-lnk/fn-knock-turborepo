import { computed, type ComputedRef, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { useConfigStore } from "../../store/config";
import type { AccessEntryInfo, RunModePromptPreferences } from "../../lib/api";
import type { ReverseProxySubmode } from "../../types";

type RunMode = 0 | 1 | 3;

type UseRunModeMessagesOptions = {
  mode: Ref<RunMode>;
  reverseProxySubmode: Ref<ReverseProxySubmode>;
  savedReverseProxySubmode: ComputedRef<ReverseProxySubmode>;
  accessEntry: Ref<AccessEntryInfo>;
  pendingPromptKey: Ref<keyof RunModePromptPreferences | null>;
  pendingSubmode: Ref<ReverseProxySubmode | null>;
};

const DEFAULT_ROUTE_PLACEHOLDER = "/__select__";

export function useRunModeMessages({
  mode,
  reverseProxySubmode,
  savedReverseProxySubmode,
  accessEntry,
  pendingPromptKey,
  pendingSubmode,
}: UseRunModeMessagesOptions) {
  const configStore = useConfigStore();
  const { locale, t } = useI18n();
  const formatInlineList = (items: string[]) =>
    items.join(locale.value === "en" ? ", " : "、");

  const proxyMappingsCount = computed(
    () => configStore.config?.proxy_mappings?.length ?? 0,
  );
  const hostMappingsCount = computed(
    () => configStore.config?.host_mappings?.length ?? 0,
  );
  const streamMappingsCount = computed(
    () => configStore.config?.stream_mappings?.length ?? 0,
  );
  const hasCustomDefaultRoute = computed(() => {
    const defaultRoute = configStore.config?.default_route?.trim() || "";
    return defaultRoute !== "" && defaultRoute !== DEFAULT_ROUTE_PLACEHOLDER;
  });

  function getRunModeLabel(
    targetMode: 0 | 1 | 3,
    targetSubmode: ReverseProxySubmode = reverseProxySubmode.value,
  ) {
    if (targetMode === 0) return t("admin.runModeSettings.directModeName");
    if (targetMode === 1) {
      return t("admin.runModeSettings.reverseModeName", {
        submode:
          targetSubmode === "subdomain"
            ? t("admin.runModeSettings.subdomainMapping")
            : t("admin.runModeSettings.pathMapping"),
      });
    }
    return t("admin.runModeSettings.subdomainModeName");
  }

  function buildFirewallResetSuccessDescription(
    result: {
      runType: 0 | 1 | 3;
      gatewayPort: number;
      exemptPorts: string[];
      whitelistSynced: number;
    },
    selectedSubmode: ReverseProxySubmode | null,
  ) {
    if (result.runType === 1) {
      return selectedSubmode === "subdomain"
        ? t("admin.runModeSettings.firewallResetReverseSubdomain")
        : t("admin.runModeSettings.firewallResetReversePath");
    }

    const exemptPortsLabel = formatInlineList(result.exemptPorts);

    if (result.runType === 0) {
      const whitelistDescription =
        result.whitelistSynced > 0
          ? t("admin.runModeSettings.firewallResetDirectWhitelistSynced", {
              count: result.whitelistSynced,
            })
          : t("admin.runModeSettings.firewallResetDirectNoWhitelist");
      return t("admin.runModeSettings.firewallResetDirect", {
        ports: exemptPortsLabel,
        whitelist: whitelistDescription,
      });
    }

    return t("admin.runModeSettings.firewallResetSubdomain", {
      ports: exemptPortsLabel,
    });
  }

  function buildUnsavedModeNotice() {
    const currentMode = configStore.config?.run_type;
    const currentSubmode = savedReverseProxySubmode.value;
    if (currentMode === undefined) return "";
    const hasChanges =
      currentMode !== mode.value ||
      (mode.value === 1 && currentSubmode !== reverseProxySubmode.value);
    if (!hasChanges) return "";
    return t("admin.runModeSettings.unsavedModeNotice", {
      current: getRunModeLabel(currentMode, currentSubmode),
      target: getRunModeLabel(mode.value, reverseProxySubmode.value),
    });
  }

  function buildRunModeChangeSuccessDescription(
    nextMode: 0 | 1 | 3,
    nextSubmode: ReverseProxySubmode | null,
  ) {
    if (nextMode === 3) {
      if (proxyMappingsCount.value > 0) {
        return t("admin.runModeSettings.successSubdomainClearedMappings", {
          count: proxyMappingsCount.value,
          defaultRoute: hasCustomDefaultRoute.value
            ? t("admin.runModeSettings.successDefaultRouteReset")
            : "",
        });
      }
      return t("admin.runModeSettings.successSubdomainNoMappings");
    }

    if (nextMode === 1) {
      if (nextSubmode === "subdomain") {
        if (proxyMappingsCount.value > 0) {
          return t(
            "admin.runModeSettings.successReverseSubdomainWithMappings",
            {
              count: proxyMappingsCount.value,
            },
          );
        }
        return t("admin.runModeSettings.successReverseSubdomainNoMappings");
      }

      const preservedItems: string[] = [];
      if (hostMappingsCount.value > 0) {
        preservedItems.push(
          t("admin.runModeSettings.hostMappingsCount", {
            count: hostMappingsCount.value,
          }),
        );
      }
      if (streamMappingsCount.value > 0) {
        preservedItems.push(
          t("admin.runModeSettings.streamMappingsCount", {
            count: streamMappingsCount.value,
          }),
        );
      }

      if (preservedItems.length > 0) {
        return t("admin.runModeSettings.successReversePathWithPreserved", {
          items: formatInlineList(preservedItems),
        });
      }

      return t("admin.runModeSettings.successReversePathNoPreserved");
    }

    return t("admin.runModeSettings.successRulesApplied");
  }

  function buildSubdomainResetMessage() {
    if (proxyMappingsCount.value === 0) {
      return t("admin.runModeSettings.subdomainResetNoMappings");
    }

    return t("admin.runModeSettings.subdomainResetWithMappings", {
      count: proxyMappingsCount.value,
      defaultRoute: hasCustomDefaultRoute.value
        ? t("admin.runModeSettings.successDefaultRouteReset")
        : "",
    });
  }

  function buildReverseProxyCompatibilityMessage(
    targetSubmode: ReverseProxySubmode,
  ) {
    if (targetSubmode === "subdomain") {
      if (proxyMappingsCount.value === 0) {
        return t("admin.runModeSettings.compatReverseSubdomainNoMappings");
      }
      return t("admin.runModeSettings.compatReverseSubdomainWithMappings", {
        count: proxyMappingsCount.value,
      });
    }

    const preservedItems: string[] = [];
    if (hostMappingsCount.value > 0) {
      preservedItems.push(
        t("admin.runModeSettings.hostMappingsCount", {
          count: hostMappingsCount.value,
        }),
      );
    }
    if (streamMappingsCount.value > 0) {
      preservedItems.push(
        t("admin.runModeSettings.streamMappingsCount", {
          count: streamMappingsCount.value,
        }),
      );
    }

    if (preservedItems.length === 0) {
      return t("admin.runModeSettings.compatReversePathNoPreserved");
    }

    return t("admin.runModeSettings.compatReversePathWithPreserved", {
      items: formatInlineList(preservedItems),
    });
  }

  const confirmDialogContent = computed(() => {
    const port = accessEntry.value.port;
    const targetSubmode = pendingSubmode.value ?? reverseProxySubmode.value;

    if (pendingPromptKey.value === "reverseProxyToDirect") {
      return {
        title: t("admin.runModeSettings.promptDirectTitle"),
        description: t("admin.runModeSettings.promptDirectDescription"),
        items: [
          t("admin.runModeSettings.promptDirectItemFirewall", { port }),
          t("admin.runModeSettings.promptDirectItemLoginEntry", { port }),
          t("admin.runModeSettings.promptDirectItemMultiEntry"),
          t("admin.runModeSettings.promptDirectItemLan"),
          t("admin.runModeSettings.promptDirectItemNoTunnel"),
        ],
      };
    }

    if (
      pendingPromptKey.value === "directToReverseProxy" ||
      pendingPromptKey.value === "subdomainToReverseProxy"
    ) {
      return {
        title: t("admin.runModeSettings.promptSwitchTo", {
          mode: getRunModeLabel(1, targetSubmode),
        }),
        description:
          targetSubmode === "subdomain"
            ? t("admin.runModeSettings.promptReverseSubdomainDescription")
            : t("admin.runModeSettings.promptReversePathDescription"),
        items: [
          buildReverseProxyCompatibilityMessage(targetSubmode),
          t("admin.runModeSettings.promptReverseItemClearFirewall"),
          targetSubmode === "subdomain"
            ? t("admin.runModeSettings.promptReverseItemSubdomainEntry", {
                port,
              })
            : t("admin.runModeSettings.promptReverseItemPathEntry", { port }),
          targetSubmode === "subdomain"
            ? t("admin.runModeSettings.promptReverseItemSubdomainUi")
            : t("admin.runModeSettings.promptReverseItemPathUi"),
        ],
      };
    }

    if (pendingPromptKey.value === "switchToSubdomain") {
      return {
        title: t("admin.runModeSettings.promptSubdomainTitle"),
        description: t("admin.runModeSettings.promptSubdomainDescription"),
        items: [
          buildSubdomainResetMessage(),
          t("admin.runModeSettings.promptSubdomainItemEntry", { port }),
          t("admin.runModeSettings.promptSubdomainItemBindLocal"),
          t("admin.runModeSettings.promptSubdomainItemAuth"),
          t("admin.runModeSettings.promptSubdomainItemIptables"),
        ],
      };
    }

    return {
      title: t("admin.runModeSettings.promptSwitchTo", {
        mode: getRunModeLabel(1, targetSubmode),
      }),
      description: t("admin.runModeSettings.promptReverseGenericDescription"),
      items: [
        buildReverseProxyCompatibilityMessage(targetSubmode),
        t("admin.runModeSettings.promptReverseItemCentralEntry"),
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.promptReverseItemSubdomainEntry", { port })
          : t("admin.runModeSettings.promptReverseItemPathEntry", { port }),
        targetSubmode === "subdomain"
          ? t("admin.runModeSettings.promptReverseItemSubdomainCompatible")
          : t("admin.runModeSettings.promptReverseItemPathServices"),
      ],
    };
  });

  return {
    buildFirewallResetSuccessDescription,
    buildRunModeChangeSuccessDescription,
    buildUnsavedModeNotice,
    confirmDialogContent,
    formatInlineList,
    getRunModeLabel,
  };
}
