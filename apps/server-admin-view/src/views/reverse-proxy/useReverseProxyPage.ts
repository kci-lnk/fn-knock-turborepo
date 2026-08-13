import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import { ConfigAPI } from "@/lib/api/config";
import { useConfigStore } from "@/store/config";
import type { ProxyMapping } from "@/types";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDefaultRouteConfirm } from "@admin-shared/composables/useDefaultRouteConfirm";
import { useLocalPagedList } from "@admin-shared/composables/useLocalPagedList";
import { useProxyMappingDialogForm } from "@admin-shared/composables/useProxyMappingDialogForm";
import { needsClearDefaultRouteConfirm, needsSetDefaultRouteConfirm } from "@admin-shared/utils/defaultRouteGuard";
import { extractPortFromTarget } from "@admin-shared/utils/extractPortFromTarget";
import { isWebSocketProxyTargetUrl } from "@admin-shared/utils/proxyTargetInput";
import { DEFAULT_PROXY_MAPPING_FLAGS } from "@admin-shared/utils/proxyMapping";
import {
  createReverseProxyMessages,
  showReverseProxyActionError,
  showReverseProxyBooleanResultToast,
} from "@admin-shared/utils/reverseProxyFeedback";
import { useReverseProxyDiscoverFlow } from "./useReverseProxyDiscoverFlow";
import { useReverseProxyMappingActions } from "./useReverseProxyMappingActions";

type DiscoverTargetsSettingsHandle = {
  ensureSaved: () => Promise<string[]> | undefined;
  loadTargets: () => Promise<void> | undefined;
};

const DEFAULT_SYSTEM_PORT = 5666;

export const useReverseProxyPage = () => {
  const { t } = useI18n();
  const configStore = useConfigStore();
  const messages = createReverseProxyMessages(t);
  const discoverTargetsSettingsRef = ref<DiscoverTargetsSettingsHandle | null>(
    null,
  );
  const isScanIntensityDialogOpen = ref(false);
  const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();

  const allMappings = computed(() => configStore.config?.proxy_mappings || []);
  const isDefaultRoute = (path: string) =>
    configStore.config?.default_route === path;

  const {
    open: isMappingDialogOpen,
    isEditing,
    editingOriginal: editingOriginalMapping,
    form: newMapping,
    isValid,
    openAdd: openAddDialog,
    openEdit: openEditDialog,
    close: closeMappingDialog,
  } = useProxyMappingDialogForm<ProxyMapping>(DEFAULT_PROXY_MAPPING_FLAGS);
  const isNewMappingWebSocketTarget = computed(() =>
    isWebSocketProxyTargetUrl(newMapping.target),
  );
  const handleMappingDialogOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) closeMappingDialog(true);
  };
  const updateMappingDraft = (patch: Partial<ProxyMapping>) => {
    Object.assign(newMapping, patch);
  };

  const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
    onError: (error) => {
      showReverseProxyActionError(
        messages.syncFailed,
        error,
        messages.networkError,
      );
    },
  });
  const { isPending: isSavingDefaultRoute, run: runSaveDefaultRoute } =
    useAsyncAction({
      onError: (error) => {
        showReverseProxyActionError(
          messages.defaultRouteUpdateFailed,
          error,
          messages.unknownError,
        );
      },
    });

  const {
    open: isDefaultRouteConfirmOpen,
    pendingPath: pendingDefaultRoutePath,
    showDefaultRouteFnosHint,
    dialogTitle: defaultRouteDialogTitle,
    dialogDescription: defaultRouteDialogDescription,
    queue: queueDefaultRouteAction,
    reset: closeDefaultRouteConfirm,
  } = useDefaultRouteConfirm(DEFAULT_SYSTEM_PORT);
  const handleDefaultRouteConfirmOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) closeDefaultRouteConfirm();
  };

  const currentDefaultRouteMapping = computed(() => {
    const currentPath = configStore.config?.default_route;
    if (!currentPath || currentPath === "/__select__") return null;
    return allMappings.value.find(({ path }) => path === currentPath) ?? null;
  });
  const currentDefaultRoutePort = computed(() => {
    if (!currentDefaultRouteMapping.value) return null;
    return extractPortFromTarget(currentDefaultRouteMapping.value.target);
  });
  const applyDefaultRoute = async (path: string) => {
    await runSaveDefaultRoute(() => configStore.saveDefaultRoute(path));
  };
  const requestClearDefaultRoute = (mapping: ProxyMapping) => {
    const targetPort = extractPortFromTarget(mapping.target);
    if (needsClearDefaultRouteConfirm(targetPort, DEFAULT_SYSTEM_PORT)) {
      queueDefaultRouteAction("/__select__", "clear", targetPort);
      return;
    }
    void applyDefaultRoute("/__select__");
  };
  const requestSetDefaultRoute = (mapping: ProxyMapping) => {
    if (
      needsSetDefaultRouteConfirm(
        currentDefaultRoutePort.value,
        currentDefaultRouteMapping.value?.path,
        mapping.path,
        DEFAULT_SYSTEM_PORT,
      )
    ) {
      queueDefaultRouteAction(
        mapping.path,
        "set",
        currentDefaultRoutePort.value,
      );
      return;
    }
    void applyDefaultRoute(mapping.path);
  };
  const confirmDefaultRouteChange = async () => {
    if (!pendingDefaultRoutePath.value) return;
    await applyDefaultRoute(pendingDefaultRoutePath.value);
    closeDefaultRouteConfirm();
  };

  const {
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    filteredItems: filteredMappings,
    pagedItems: paginatedMappings,
    handlePageChange,
    handleLimitChange,
  } = useLocalPagedList<ProxyMapping>({
    items: allMappings,
    normalizeQuery: (query) => query.toLowerCase(),
    filter: (mapping, query) =>
      mapping.path.toLowerCase().includes(query) ||
      mapping.target.toLowerCase().includes(query),
  });
  const setSearchQuery = (value: string) => {
    searchQuery.value = value;
  };

  const { isSaving, removeMapping, removingPath, runSaveAction, saveMapping } =
    useReverseProxyMappingActions({
      allMappings,
      closeMappingDialog,
      currentPage,
      editingOriginalMapping,
      form: newMapping,
      isDefaultRoute,
      isEditing,
      isValid,
      messages,
      paginatedMappings,
      saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
      saveProxyMappings: (mappings) =>
        configStore.saveProxyMappings(mappings),
      searchQuery,
    });

  const {
    discoveredData,
    dismissDiscoverDialog,
    handleDiscoverDialogOpenChange,
    isAllSelected,
    isDiscoverDialogOpen,
    isDiscovering,
    isDiscoverSelectionValid,
    isDiscoverSettingsOpen,
    onToggleAllDiscoverSelect,
    openDiscoverDialog,
    resolveDiscoveredServiceHost,
    saveDiscoveredServices,
    selectedServices,
    showDiscoverHostColumn,
    stopDiscoverScan,
    toggleDiscoverSettings,
    triggerScan,
  } = useReverseProxyDiscoverFlow({
    allMappings,
    currentHostname: window.location.hostname,
    currentPage,
    discoverTargetsSettingsRef,
    messages,
    runSaveAction,
    saveDefaultRoute: (path) => configStore.saveDefaultRoute(path),
    saveProxyMappings: (mappings) => configStore.saveProxyMappings(mappings),
    searchQuery,
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });

  const setDiscoverTargetsSettingsRef = (handle: unknown) => {
    discoverTargetsSettingsRef.value =
      handle as DiscoverTargetsSettingsHandle | null;
  };
  const syncRoutes = async () => {
    await runSyncRoutes(() => ConfigAPI.syncRoutes(), {
      onSuccess: (result) => {
        showReverseProxyBooleanResultToast(result, {
          successText: messages.syncSuccess(result.data?.synced_rules ?? 0),
          errorText: messages.syncFailed,
          unknownErrorText: messages.unknownError,
        });
      },
    });
  };

  onMounted(() => void loadAccessEntryPort());
  onUnmounted(stopDiscoverScan);

  return reactive({
    accessEntryPort,
    closeMappingDialog,
    closeDefaultRouteConfirm,
    confirmDefaultRouteChange,
    currentPage,
    defaultRouteDialogDescription,
    defaultRouteDialogTitle,
    discoveredData,
    dismissDiscoverDialog,
    filteredMappings,
    handleDefaultRouteConfirmOpenChange,
    handleDiscoverDialogOpenChange,
    handleLimitChange,
    handleMappingDialogOpenChange,
    handlePageChange,
    isAllSelected,
    isDefaultRoute,
    isDefaultRouteConfirmOpen,
    isDiscoverDialogOpen,
    isDiscovering,
    isDiscoverSelectionValid,
    isDiscoverSettingsOpen,
    isEditing,
    isMappingDialogOpen,
    isNewMappingWebSocketTarget,
    isSaving,
    isSavingDefaultRoute,
    isScanIntensityDialogOpen,
    isSyncing,
    isValid,
    limit,
    newMapping,
    onToggleAllDiscoverSelect,
    openAddDialog,
    openDiscoverDialog,
    openEditDialog,
    paginatedMappings,
    parsedLimit,
    removeMapping,
    removingPath,
    requestClearDefaultRoute,
    requestSetDefaultRoute,
    resolveDiscoveredServiceHost,
    saveDiscoveredServices,
    saveMapping,
    searchQuery,
    selectedServices,
    setDiscoverTargetsSettingsRef,
    setSearchQuery,
    showDefaultRouteFnosHint,
    showDiscoverHostColumn,
    stopDiscoverScan,
    syncRoutes,
    toggleDiscoverSettings,
    triggerScan,
    updateMappingDraft,
  });
};

export type ReverseProxyPageModel = ReturnType<typeof useReverseProxyPage>;
