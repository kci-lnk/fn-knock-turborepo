import { computed, ref, watch, type Ref } from "vue";
import { ConfigAPI } from "@/lib/api/config";
import type {
  AppConfig,
  GatewayHostResponseDetails,
  GatewayProxyHeadersDetails,
  HostMapping,
} from "@/types";
import { isProxyHostMapping } from "@/lib/host-mapping-target";
import {
  HOME_ASSISTANT_TARGET_PORT,
  hasSameDisabledHosts,
  mergeGatewayDisabledHostsForMapping,
  normalizeDisabledHosts,
  normalizeHostLike,
  parseTargetPort,
} from "./model";

type Translate = (key: string) => string;

export const useMappingGatewayAdvanced = ({
  getConfig,
  getErrorMessage,
  isDialogOpen,
  isGatewayAdvancedAvailableByMode,
  isMappingAuthService,
  isMappingProxy,
  mappingDraftHost,
  setGatewayHostResponseDisabledHosts,
  setGatewayProxyHeadersDisabledHosts,
  translate,
  visibleMappings,
}: {
  getConfig: () => AppConfig | null | undefined;
  getErrorMessage: (error: unknown, fallback: string) => string;
  isDialogOpen: Ref<boolean>;
  isGatewayAdvancedAvailableByMode: Ref<boolean>;
  isMappingAuthService: Ref<boolean>;
  isMappingProxy: Ref<boolean>;
  mappingDraftHost: Ref<string>;
  setGatewayHostResponseDisabledHosts: (disabledHosts: string[]) => void;
  setGatewayProxyHeadersDisabledHosts: (disabledHosts: string[]) => void;
  translate: Translate;
  visibleMappings: Ref<HostMapping[]>;
}) => {
  const gatewayProxyHeadersDetails = ref<GatewayProxyHeadersDetails | null>(
    null,
  );
  const gatewayHostResponseDetails = ref<GatewayHostResponseDetails | null>(
    null,
  );
  const isLoadingGatewayProxyHeaders = ref(false);
  const isLoadingGatewayHostResponse = ref(false);
  const gatewayProxyHeadersLoadError = ref("");
  const gatewayHostResponseLoadError = ref("");
  const sendProxyHeaders = ref(true);
  const preserveHost = ref(true);
  const sendProxyHeadersTouched = ref(false);
  const preserveHostTouched = ref(false);
  const mappingAdvancedCleanupHosts = ref<string[]>([]);
  const isGatewayAdvancedLoading = computed(
    () =>
      isMappingProxy.value &&
      (isLoadingGatewayProxyHeaders.value ||
        isLoadingGatewayHostResponse.value),
  );

  let gatewayProxyHeadersRequestId = 0;
  let gatewayHostResponseRequestId = 0;

  const sendProxyHeadersModel = computed({
    get: () => sendProxyHeaders.value,
    set: (value: boolean) => {
      sendProxyHeadersTouched.value = true;
      sendProxyHeaders.value = value;
    },
  });

  const preserveHostModel = computed({
    get: () => preserveHost.value,
    set: (value: boolean) => {
      preserveHostTouched.value = true;
      preserveHost.value = value;
    },
  });

  const gatewayProxyHeadersBlockedReason = computed(() => {
    if (isMappingAuthService.value)
      return translate("admin.subdomainProxy.proxyHeadersAuthBlocked");
    if (isLoadingGatewayProxyHeaders.value)
      return translate("admin.subdomainProxy.proxyHeadersLoading");
    if (gatewayProxyHeadersLoadError.value) {
      return gatewayProxyHeadersLoadError.value;
    }
    if (gatewayProxyHeadersDetails.value) {
      return gatewayProxyHeadersDetails.value.availability.available
        ? ""
        : gatewayProxyHeadersDetails.value.availability.reason;
    }
    if (!isGatewayAdvancedAvailableByMode.value) {
      return translate("admin.subdomainProxy.proxyHeadersModeBlocked");
    }
    return "";
  });

  const gatewayHostResponseBlockedReason = computed(() => {
    if (isMappingAuthService.value)
      return translate("admin.subdomainProxy.hostResponseAuthBlocked");
    if (isLoadingGatewayHostResponse.value)
      return translate("admin.subdomainProxy.hostResponseLoading");
    if (gatewayHostResponseLoadError.value) {
      return gatewayHostResponseLoadError.value;
    }
    if (gatewayHostResponseDetails.value) {
      return gatewayHostResponseDetails.value.availability.available
        ? ""
        : gatewayHostResponseDetails.value.availability.reason;
    }
    if (!isGatewayAdvancedAvailableByMode.value) {
      return translate("admin.subdomainProxy.hostResponseModeBlocked");
    }
    return "";
  });

  const hasProtocolHeadersSensitiveMappings = computed(() =>
    visibleMappings.value.some(
      (mapping) =>
        isProxyHostMapping(mapping) &&
        parseTargetPort(mapping.target) === HOME_ASSISTANT_TARGET_PORT,
    ),
  );

  const listedGatewayProxyHeaderTargets = computed(() => {
    const targets = new Set<string>();

    for (const item of gatewayProxyHeadersDetails.value?.items ?? []) {
      const target = item.target.trim();
      if (target) {
        targets.add(target);
      }
    }

    return targets;
  });

  const disabledGatewayProxyHeaderTargets = computed(() => {
    const targets = new Set<string>();
    const disabledHosts = new Set(
      normalizeDisabledHosts(
        gatewayProxyHeadersDetails.value?.config.disabled_hosts ??
          getConfig()?.gateway_proxy_headers?.disabled_hosts,
      ),
    );

    for (const mapping of visibleMappings.value) {
      if (!isProxyHostMapping(mapping)) continue;
      const target = mapping.target.trim();
      if (target && disabledHosts.has(normalizeHostLike(mapping.host))) {
        targets.add(target);
      }
    }

    if (gatewayProxyHeadersDetails.value) {
      for (const item of gatewayProxyHeadersDetails.value.items) {
        const target = item.target.trim();
        if (target && item.send_proxy_headers === false) {
          targets.add(target);
        }
      }
      return targets;
    }

    return targets;
  });

  const visibleMappingsSignature = computed(() =>
    visibleMappings.value
      .map(
        (mapping) =>
          `${normalizeHostLike(mapping.host)}::${isProxyHostMapping(mapping) ? mapping.target.trim() : "static"}`,
      )
      .join("|"),
  );

  const cancelGatewayProxyHeadersLoad = () => {
    gatewayProxyHeadersRequestId += 1;
    isLoadingGatewayProxyHeaders.value = false;
  };

  const cancelGatewayHostResponseLoad = () => {
    gatewayHostResponseRequestId += 1;
    isLoadingGatewayHostResponse.value = false;
  };

  const resolveSendProxyHeadersForHost = (host: string): boolean => {
    const normalizedHost = normalizeHostLike(host);
    if (!normalizedHost) return true;

    const disabledHosts = new Set(
      normalizeDisabledHosts(
        gatewayProxyHeadersDetails.value?.config.disabled_hosts ??
          getConfig()?.gateway_proxy_headers?.disabled_hosts,
      ),
    );
    return !disabledHosts.has(normalizedHost);
  };

  const resolvePreserveHostForHost = (host: string): boolean => {
    const normalizedHost = normalizeHostLike(host);
    if (!normalizedHost) return true;

    const disabledHosts = new Set(
      normalizeDisabledHosts(
        gatewayHostResponseDetails.value?.config.disabled_hosts ??
          getConfig()?.gateway_host_response?.disabled_hosts,
      ),
    );
    return !disabledHosts.has(normalizedHost);
  };

  const applyMappingGatewayDraftFromConfig = (
    host = mappingDraftHost.value,
  ) => {
    const normalizedHost = normalizeHostLike(host);
    if (!sendProxyHeadersTouched.value) {
      sendProxyHeaders.value = resolveSendProxyHeadersForHost(normalizedHost);
    }
    if (!preserveHostTouched.value) {
      preserveHost.value = resolvePreserveHostForHost(normalizedHost);
    }
  };

  const applyGatewayProxyHeadersDetails = (
    details: GatewayProxyHeadersDetails,
  ) => {
    gatewayProxyHeadersDetails.value = details;
    setGatewayProxyHeadersDisabledHosts([...details.config.disabled_hosts]);
    applyMappingGatewayDraftFromConfig();
  };

  const applyGatewayHostResponseDetails = (
    details: GatewayHostResponseDetails,
  ) => {
    gatewayHostResponseDetails.value = details;
    setGatewayHostResponseDisabledHosts([...details.config.disabled_hosts]);
    applyMappingGatewayDraftFromConfig();
  };

  const resetGatewayAdvancedState = (host = "") => {
    mappingAdvancedCleanupHosts.value = [];
    sendProxyHeadersTouched.value = false;
    preserveHostTouched.value = false;
    sendProxyHeaders.value = resolveSendProxyHeadersForHost(host);
    preserveHost.value = resolvePreserveHostForHost(host);
    gatewayProxyHeadersLoadError.value = "";
    gatewayHostResponseLoadError.value = "";
  };

  const addMappingAdvancedCleanupHost = (host: string | null) => {
    const normalizedHost = host ? normalizeHostLike(host) : "";
    if (!normalizedHost) return;
    if (mappingAdvancedCleanupHosts.value.includes(normalizedHost)) return;
    mappingAdvancedCleanupHosts.value = [
      ...mappingAdvancedCleanupHosts.value,
      normalizedHost,
    ];
  };

  const collectMappingAdvancedCleanupHosts = (
    previousHost: string | null,
  ): string[] =>
    normalizeDisabledHosts([
      ...mappingAdvancedCleanupHosts.value,
      ...(previousHost ? [previousHost] : []),
    ]);

  const loadGatewayProxyHeadersDetails = async (
    options: { force?: boolean; trackLoading?: boolean } = {},
  ) => {
    const requestId = ++gatewayProxyHeadersRequestId;

    if (!options.force && !hasProtocolHeadersSensitiveMappings.value) {
      gatewayProxyHeadersDetails.value = null;
      return;
    }

    if (options.trackLoading) {
      isLoadingGatewayProxyHeaders.value = true;
      gatewayProxyHeadersLoadError.value = "";
    }

    try {
      const details = await ConfigAPI.getGatewayProxyHeaders();
      if (requestId !== gatewayProxyHeadersRequestId) {
        return;
      }
      applyGatewayProxyHeadersDetails(details);
    } catch (error) {
      if (requestId !== gatewayProxyHeadersRequestId) {
        return;
      }
      if (options.trackLoading) {
        gatewayProxyHeadersLoadError.value = getErrorMessage(
          error,
          translate("admin.subdomainProxy.proxyHeadersLoadFailed"),
        );
      }
      console.warn("load gateway proxy headers failed:", error);
    } finally {
      if (options.trackLoading && requestId === gatewayProxyHeadersRequestId) {
        isLoadingGatewayProxyHeaders.value = false;
      }
    }
  };

  const loadGatewayHostResponseDetails = async (
    options: { trackLoading?: boolean } = {},
  ) => {
    const requestId = ++gatewayHostResponseRequestId;

    if (options.trackLoading) {
      isLoadingGatewayHostResponse.value = true;
      gatewayHostResponseLoadError.value = "";
    }

    try {
      const details = await ConfigAPI.getGatewayHostResponse();
      if (requestId !== gatewayHostResponseRequestId) {
        return;
      }
      applyGatewayHostResponseDetails(details);
    } catch (error) {
      if (requestId !== gatewayHostResponseRequestId) {
        return;
      }
      if (options.trackLoading) {
        gatewayHostResponseLoadError.value = getErrorMessage(
          error,
          translate("admin.subdomainProxy.hostResponseLoadFailed"),
        );
      }
      console.warn("load gateway host response failed:", error);
    } finally {
      if (options.trackLoading && requestId === gatewayHostResponseRequestId) {
        isLoadingGatewayHostResponse.value = false;
      }
    }
  };

  const loadGatewayAdvancedDetails = async () => {
    if (!isMappingProxy.value) {
      cancelGatewayProxyHeadersLoad();
      cancelGatewayHostResponseLoad();
      return;
    }
    await Promise.all([
      loadGatewayProxyHeadersDetails({ force: true, trackLoading: true }),
      loadGatewayHostResponseDetails({ trackLoading: true }),
    ]);
  };

  const saveMappingGatewayAdvanced = async (
    normalized: HostMapping,
    previousHost: string | null,
  ) => {
    const configureProxy =
      normalized.service_role !== "auth" && isProxyHostMapping(normalized);
    const nextConfigHost = configureProxy ? normalized.host : "";
    const cleanupHosts = collectMappingAdvancedCleanupHosts(previousHost);
    const currentProxyDisabledHosts = normalizeDisabledHosts(
      gatewayProxyHeadersDetails.value?.config.disabled_hosts ??
        getConfig()?.gateway_proxy_headers?.disabled_hosts,
    );
    const currentHostResponseDisabledHosts = normalizeDisabledHosts(
      gatewayHostResponseDetails.value?.config.disabled_hosts ??
        getConfig()?.gateway_host_response?.disabled_hosts,
    );
    const nextProxyDisabledHosts = mergeGatewayDisabledHostsForMapping(
      currentProxyDisabledHosts,
      cleanupHosts,
      nextConfigHost,
      configureProxy ? sendProxyHeaders.value : true,
    );
    const nextHostResponseDisabledHosts = mergeGatewayDisabledHostsForMapping(
      currentHostResponseDisabledHosts,
      cleanupHosts,
      nextConfigHost,
      configureProxy ? preserveHost.value : true,
    );
    const shouldUpdateProxyHeaders = !hasSameDisabledHosts(
      currentProxyDisabledHosts,
      nextProxyDisabledHosts,
    );
    const shouldUpdateHostResponse = !hasSameDisabledHosts(
      currentHostResponseDisabledHosts,
      nextHostResponseDisabledHosts,
    );

    if (shouldUpdateProxyHeaders) {
      cancelGatewayProxyHeadersLoad();
      const details = await ConfigAPI.updateGatewayProxyHeaders({
        disabled_hosts: nextProxyDisabledHosts,
      });
      applyGatewayProxyHeadersDetails(details);
    }

    if (shouldUpdateHostResponse) {
      cancelGatewayHostResponseLoad();
      const details = await ConfigAPI.updateGatewayHostResponse({
        disabled_hosts: nextHostResponseDisabledHosts,
      });
      applyGatewayHostResponseDetails(details);
    }
  };

  const shouldShowProtocolHeadersWarning = (mapping: HostMapping): boolean => {
    if (!isProxyHostMapping(mapping)) return false;
    const target = mapping.target.trim();
    if (!target || parseTargetPort(target) !== HOME_ASSISTANT_TARGET_PORT) {
      return false;
    }

    if (
      gatewayProxyHeadersDetails.value &&
      !listedGatewayProxyHeaderTargets.value.has(target)
    ) {
      return false;
    }

    return !disabledGatewayProxyHeaderTargets.value.has(target);
  };

  watch(isMappingProxy, (proxy) => {
    if (!isDialogOpen.value) return;
    if (proxy) void loadGatewayAdvancedDetails();
    else {
      cancelGatewayProxyHeadersLoad();
      cancelGatewayHostResponseLoad();
    }
  });

  watch(
    visibleMappingsSignature,
    () => {
      void loadGatewayProxyHeadersDetails();
    },
    { immediate: true },
  );

  watch(
    [mappingDraftHost, gatewayProxyHeadersDetails, gatewayHostResponseDetails],
    () => {
      if (isDialogOpen.value) {
        applyMappingGatewayDraftFromConfig();
      }
    },
  );

  return {
    gatewayHostResponseBlockedReason,
    isGatewayAdvancedLoading,
    gatewayProxyHeadersBlockedReason,
    loadGatewayAdvancedDetails,
    preserveHost,
    preserveHostModel,
    resetGatewayAdvancedState,
    saveMappingGatewayAdvanced,
    sendProxyHeaders,
    sendProxyHeadersModel,
    shouldShowProtocolHeadersWarning,
    addMappingAdvancedCleanupHost,
  };
};
