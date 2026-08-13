import { computed, reactive, ref, type ComputedRef, type Ref } from "vue";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api";
import type { AppConfig, HostMapping } from "@/types";
import {
  canRefreshHostMappingMetadata,
  createDefaultMapping,
  DEFAULT_PROTOCOL_MODE,
  isMappingDraftValid,
  normalizeMappingBasicAuth,
  normalizeMappingForm,
  normalizeMappingVisibility,
  type TranslationParams,
} from "./model";
import { useBasicAuthProbe } from "./useBasicAuthProbe";
import { useMappingDialogKeyboardScroll } from "./useMappingDialogKeyboardScroll";
import { useMappingGatewayAdvanced } from "./useMappingGatewayAdvanced";
import { useMappingIcon } from "./useMappingIcon";
import {
  shouldBlockMappingSaveForVisibility,
  shouldReturnToVisibilityAfterSaveError,
  useMappingVisibility,
} from "./useMappingVisibility";
import { useSubdomainMappingDraft } from "./useSubdomainMappingDraft";

type AsyncActionRun = <T>(
  action: () => Promise<T>,
  hooks?: {
    onSuccess?: (result: T) => void | Promise<void>;
    onError?: (error: unknown) => void;
  },
) => Promise<T | undefined>;

type Translate = (key: string, params?: TranslationParams) => string;

export const useSubdomainMappingDialogController = ({
  allMappings,
  canUseRootDomainSuffix,
  getConfig,
  isAuthServiceTarget,
  isGatewayAdvancedAvailableByMode,
  resetFaviconErrors,
  runSaveMappings,
  saveHostMappings,
  savedRootDomain,
  setGatewayHostResponseDisabledHosts,
  setGatewayProxyHeadersDisabledHosts,
  translate,
  visibleMappings,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  canUseRootDomainSuffix: ComputedRef<boolean>;
  getConfig: () => AppConfig | null | undefined;
  isAuthServiceTarget: (target: string) => boolean;
  isGatewayAdvancedAvailableByMode: Ref<boolean>;
  resetFaviconErrors: () => void;
  runSaveMappings: AsyncActionRun;
  saveHostMappings: (
    mappings: HostMapping[],
    refreshedFaviconHosts?: ReadonlySet<string>,
    refreshedTitleHosts?: ReadonlySet<string>,
    previousHosts?: ReadonlyMap<string, string>,
  ) => Promise<unknown>;
  savedRootDomain: ComputedRef<string>;
  setGatewayHostResponseDisabledHosts: (disabledHosts: string[]) => void;
  setGatewayProxyHeadersDisabledHosts: (disabledHosts: string[]) => void;
  translate: Translate;
  visibleMappings: ComputedRef<HostMapping[]>;
}) => {
  const isDialogOpen = ref(false);
  const editingHost = ref<string | null>(null);
  const mappingMetadataTarget = ref("");
  const faviconMetadataTarget = ref("");
  const titleRefreshDirty = ref(false);
  const mappingForm = reactive<HostMapping>(createDefaultMapping());
  let metadataRefreshRequestId = 0;

  const {
    clearMappingDialogKeyboardScrollTimer,
    handleMappingDialogFocusIn,
    handleMappingDialogViewportResize,
    mappingDialogContentStyle,
    mappingDialogScrollStyle,
    resetMappingDialogKeyboardScroll,
    setMappingDialogScrollElement,
  } = useMappingDialogKeyboardScroll({
    isDialogOpen,
  });

  const {
    composedPreviewHost,
    fullHostInputHint,
    handleMappingInputModeChange,
    mappingDraftHost,
    mappingInputLabel,
    mappingInputMode,
    mappingModeDescription,
    mappingSubdomain,
    resetMappingDraftInput,
    setMappingDraftInputFromHost,
    setMappingSubdomain,
  } = useSubdomainMappingDraft({
    canUseRootDomainSuffix,
    onSubdomainExtractionMiss: (domain) => {
      toast.info(translate("admin.subdomainProxy.switchedToSuffixMode"), {
        description: translate(
          "admin.subdomainProxy.switchedToSuffixDescription",
          { domain },
        ),
      });
    },
    savedRootDomain,
    translate,
  });

  const isMappingAuthService = computed(() =>
    isAuthServiceTarget(mappingForm.target),
  );
  const isMappingWebSocketTarget = computed(() =>
    isWebSocketProxyTargetUrl(mappingForm.target),
  );
  const basicAuthProbeEnabled = computed(
    () => isDialogOpen.value && !isMappingAuthService.value,
  );
  const mappingResolvedTitle = computed(() =>
    mappingMetadataTarget.value === mappingForm.target.trim()
      ? mappingForm.title.trim()
      : "",
  );
  const canRefreshMappingMetadata = computed(() =>
    canRefreshHostMappingMetadata(mappingForm.target),
  );

  const { currentBasicAuthProbeResult } = useBasicAuthProbe({
    enabled: basicAuthProbeEnabled,
    getErrorMessage: (error) =>
      extractErrorMessage(
        error,
        translate("admin.subdomainProxy.basicAuthProbeFailed"),
      ),
    probe: (target) => ConfigAPI.probeHostMappingBasicAuth(target),
    target: computed(() => mappingForm.target),
  });

  const showToolbar = computed({
    get: () => !isMappingWebSocketTarget.value && !mappingForm.suppress_toolbar,
    set: (value: boolean) => {
      if (isMappingWebSocketTarget.value) {
        mappingForm.suppress_toolbar = true;
        return;
      }
      mappingForm.suppress_toolbar = !value;
    },
  });
  const mappingUseAuth = computed({
    get: () => !isMappingAuthService.value && mappingForm.use_auth,
    set: (value: boolean) => {
      mappingForm.use_auth = value;
    },
  });
  const basicAuthInjectionModel = computed({
    get: () => !isMappingAuthService.value && mappingForm.basic_auth.enabled,
    set: (value: boolean) => {
      mappingForm.basic_auth.enabled = value;
      if (!value) {
        mappingForm.basic_auth.username = "";
        mappingForm.basic_auth.password = "";
      }
    },
  });
  const basicAuthValidationMessage = computed(() => {
    if (!basicAuthInjectionModel.value) return "";
    const username = mappingForm.basic_auth.username.trim();
    if (!username || !mappingForm.basic_auth.password) {
      return translate("admin.subdomainProxy.basicAuthMissing");
    }
    if (username.includes(":")) {
      return translate("admin.subdomainProxy.basicAuthUsernameColon");
    }
    return "";
  });
  const canShowBasicAuthInjection = computed(
    () =>
      !isMappingAuthService.value &&
      (basicAuthInjectionModel.value ||
        currentBasicAuthProbeResult.value?.requiresBasicAuth === true),
  );

  const mappingVisibility = useMappingVisibility({
    isDialogOpen,
    isMappingAuthService,
    mappingForm,
    translate,
  });
  const visibilityEditor = reactive(mappingVisibility);
  const mappingIcon = useMappingIcon({
    canRefreshMetadata: canRefreshMappingMetadata,
    getMetadataBasicAuth: getMappingMetadataBasicAuth,
    isDialogOpen,
    mappingForm,
    metadataTarget: faviconMetadataTarget,
    resetFaviconErrors,
    translate,
  });
  const iconEditor = reactive(mappingIcon);

  const {
    addMappingAdvancedCleanupHost,
    gatewayHostResponseBlockedReason,
    gatewayProxyHeadersBlockedReason,
    isGatewayAdvancedLoading,
    loadGatewayAdvancedDetails,
    preserveHostModel,
    resetGatewayAdvancedState,
    saveMappingGatewayAdvanced,
    sendProxyHeadersModel,
    shouldShowProtocolHeadersWarning,
  } = useMappingGatewayAdvanced({
    getConfig,
    getErrorMessage: extractErrorMessage,
    isDialogOpen,
    isGatewayAdvancedAvailableByMode,
    isMappingAuthService,
    mappingDraftHost,
    setGatewayHostResponseDisabledHosts,
    setGatewayProxyHeadersDisabledHosts,
    translate: (key) => translate(key),
    visibleMappings,
  });

  const isMappingValid = computed(
    () =>
      !shouldBlockMappingSaveForVisibility({
        isAuthService: isMappingAuthService.value,
        isLoading: mappingVisibility.isGlobalVisibilityLoading.value,
        loadError: mappingVisibility.globalVisibilityLoadError.value,
        validationMessage: mappingVisibility.visibilityValidationMessage.value,
      }) &&
      isMappingDraftValid({
        basicAuthValidationMessage: basicAuthValidationMessage.value,
        canUseRootDomainSuffix: canUseRootDomainSuffix.value,
        host: mappingDraftHost.value,
        inputMode: mappingInputMode.value,
        target: mappingForm.target,
      }),
  );

  const {
    isPending: isRefreshingMappingMetadata,
    run: runRefreshMappingMetadata,
  } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.subdomainProxy.refreshFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.subdomainProxy.refreshMetadataFailed"),
        ),
      });
    },
  });

  function updateMappingForm(patch: Partial<HostMapping>) {
    Object.assign(mappingForm, patch);
  }

  function updateMappingBasicAuth(patch: Partial<HostMapping["basic_auth"]>) {
    Object.assign(mappingForm.basic_auth, patch);
  }

  function setMappingUseAuth(value: boolean) {
    mappingUseAuth.value = value;
  }

  function setShowToolbar(value: boolean) {
    showToolbar.value = value;
  }

  function setBasicAuthInjection(value: boolean) {
    basicAuthInjectionModel.value = value;
  }

  function setSendProxyHeaders(value: boolean) {
    sendProxyHeadersModel.value = value;
  }

  function setPreserveHost(value: boolean) {
    preserveHostModel.value = value;
  }

  function resetMappingAdvancedState(host = "") {
    metadataRefreshRequestId += 1;
    titleRefreshDirty.value = false;
    resetGatewayAdvancedState(host);
    mappingVisibility.resetVisibilityEditor();
    mappingIcon.resetIconEditor();
  }

  function openCreateDialog(groupId: string | null = null) {
    editingHost.value = null;
    resetMappingDraftInput();
    mappingMetadataTarget.value = "";
    faviconMetadataTarget.value = "";
    Object.assign(mappingForm, {
      ...createDefaultMapping(),
      group_id: groupId,
    });
    resetMappingAdvancedState("");
    isDialogOpen.value = true;
    void Promise.all([
      loadGatewayAdvancedDetails(),
      mappingVisibility.loadGlobalVisibility(),
    ]);
  }

  function openEditDialog(mapping: HostMapping) {
    editingHost.value = mapping.host;
    setMappingDraftInputFromHost(mapping.host);
    Object.assign(mappingForm, {
      ...mapping,
      favicon_override: mapping.favicon_override ?? "",
      target_path_mode: mapping.target_path_mode || "entry",
      protocol_mode: mapping.protocol_mode || DEFAULT_PROTOCOL_MODE,
      basic_auth: normalizeMappingBasicAuth(mapping.basic_auth),
      visibility: normalizeMappingVisibility(mapping.visibility),
    });
    mappingMetadataTarget.value = mapping.target.trim();
    faviconMetadataTarget.value = mapping.target.trim();
    resetMappingAdvancedState(mapping.host);
    isDialogOpen.value = true;
    void Promise.all([
      loadGatewayAdvancedDetails(),
      mappingVisibility.loadGlobalVisibility(),
    ]);
  }

  function closeDialog() {
    resetMappingDialogKeyboardScroll();
    isDialogOpen.value = false;
    editingHost.value = null;
    resetMappingDraftInput();
    mappingMetadataTarget.value = "";
    faviconMetadataTarget.value = "";
    Object.assign(mappingForm, createDefaultMapping());
    resetMappingAdvancedState("");
  }

  function handleDialogOpenChange(nextOpen: boolean) {
    if (!nextOpen) {
      closeDialog();
    }
  }

  function getMappingMetadataBasicAuth(): HostMapping["basic_auth"] | null {
    if (!basicAuthInjectionModel.value || basicAuthValidationMessage.value) {
      return null;
    }

    const basicAuth = normalizeMappingBasicAuth(mappingForm.basic_auth);
    return basicAuth.enabled ? basicAuth : null;
  }

  async function refreshMappingMetadata() {
    if (!canRefreshMappingMetadata.value) return;
    const requestId = ++metadataRefreshRequestId;
    const target = mappingForm.target.trim();

    await runRefreshMappingMetadata(
      () =>
        ConfigAPI.fetchHostMappingMetadata(
          target,
          getMappingMetadataBasicAuth(),
        ),
      {
        onSuccess: (metadata) => {
          if (
            requestId !== metadataRefreshRequestId ||
            mappingForm.target.trim() !== target
          ) {
            return;
          }
          mappingMetadataTarget.value = target;
          mappingForm.title = metadata.title.trim();
          titleRefreshDirty.value = true;
          toast.success(translate("admin.subdomainProxy.metadataRefreshed"), {
            description: metadata.title.trim()
              ? translate("admin.subdomainProxy.fetchedTitle", {
                  title: metadata.title.trim(),
                })
              : translate("admin.subdomainProxy.metadataNoTitle"),
          });
        },
      },
    );
  }

  async function saveMapping() {
    if (!isMappingValid.value) return;
    if (isGatewayAdvancedLoading.value) return;
    if (mappingIcon.isIconBusy.value) return;

    const normalized = normalizeMappingForm(mappingForm, {
      hasFreshTitleMetadata:
        mappingMetadataTarget.value === mappingForm.target.trim(),
      hasFreshFaviconMetadata:
        faviconMetadataTarget.value === mappingForm.target.trim(),
      host: mappingDraftHost.value,
      isAuthServiceTarget,
      isWebSocketTarget: isWebSocketProxyTargetUrl,
    });
    const duplicateHost = allMappings.value.find(
      (item) =>
        item.host === normalized.host && item.host !== editingHost.value,
    );
    if (duplicateHost) {
      toast.error(translate("admin.subdomainProxy.hostExists"), {
        description: translate("admin.subdomainProxy.hostExistsDescription", {
          host: normalized.host,
        }),
      });
      return;
    }

    const duplicateAuthService = allMappings.value.find(
      (item) =>
        isAuthServiceTarget(item.target) && item.host !== editingHost.value,
    );
    if (normalized.service_role === "auth" && duplicateAuthService) {
      toast.error(translate("admin.subdomainProxy.authServiceExists"), {
        description: translate(
          "admin.subdomainProxy.duplicateAuthServiceDescription",
          { host: duplicateAuthService.host },
        ),
      });
      return;
    }

    await runSaveMappings(
      async () => {
        const next = [...allMappings.value];
        const previousHost = editingHost.value;
        const index = editingHost.value
          ? next.findIndex((item) => item.host === editingHost.value)
          : -1;

        if (index >= 0) {
          next[index] = normalized;
        } else {
          next.push(normalized);
        }

        await saveHostMappings(
          next,
          mappingIcon.faviconRefreshDirty.value &&
            faviconMetadataTarget.value === normalized.target.trim()
            ? new Set([normalized.host])
            : undefined,
          titleRefreshDirty.value &&
            mappingMetadataTarget.value === normalized.target.trim()
            ? new Set([normalized.host])
            : undefined,
          previousHost && previousHost !== normalized.host
            ? new Map([[normalized.host, previousHost]])
            : undefined,
        );
        if (previousHost !== normalized.host) {
          addMappingAdvancedCleanupHost(previousHost);
        }
        editingHost.value = normalized.host;
        Object.assign(mappingForm, normalized);

        try {
          await saveMappingGatewayAdvanced(normalized, previousHost);
        } catch (error) {
          toast.error(translate("admin.subdomainProxy.advancedSaveFailed"), {
            description: extractErrorMessage(
              error,
              translate("admin.subdomainProxy.advancedConfigSaveFailed"),
            ),
          });
          return;
        }

        toast.success(
          index >= 0
            ? translate("admin.subdomainProxy.mappingUpdated")
            : translate("admin.subdomainProxy.mappingAdded"),
        );
        closeDialog();
      },
      {
        onError: () => {
          if (
            shouldReturnToVisibilityAfterSaveError(normalized.visibility.mode)
          ) {
            mappingVisibility.openVisibilityView();
          }
        },
      },
    );
  }

  return {
    ...mappingVisibility,
    basicAuthInjectionModel,
    basicAuthValidationMessage,
    canRefreshMappingMetadata,
    canShowBasicAuthInjection,
    clearMappingDialogKeyboardScrollTimer,
    closeDialog,
    composedPreviewHost,
    fullHostInputHint,
    gatewayHostResponseBlockedReason,
    gatewayProxyHeadersBlockedReason,
    handleDialogOpenChange,
    handleMappingDialogFocusIn,
    handleMappingDialogViewportResize,
    handleMappingInputModeChange,
    isGatewayAdvancedLoading,
    iconEditor,
    isDialogOpen,
    isMappingAuthService,
    isMappingValid,
    isMappingWebSocketTarget,
    isRefreshingMappingMetadata,
    mappingDialogContentStyle,
    mappingDialogScrollStyle,
    mappingForm,
    mappingInputLabel,
    mappingInputMode,
    mappingModeDescription,
    mappingResolvedTitle,
    mappingSubdomain,
    mappingUseAuth,
    openCreateDialog,
    openEditDialog,
    preserveHostModel,
    refreshMappingMetadata,
    saveMapping,
    sendProxyHeadersModel,
    setBasicAuthInjection,
    setMappingDialogScrollElement,
    setMappingSubdomain,
    setMappingUseAuth,
    setPreserveHost,
    setSendProxyHeaders,
    setShowToolbar,
    shouldShowProtocolHeadersWarning,
    showToolbar,
    updateMappingBasicAuth,
    updateMappingForm,
    visibilityEditor,
  };
};
