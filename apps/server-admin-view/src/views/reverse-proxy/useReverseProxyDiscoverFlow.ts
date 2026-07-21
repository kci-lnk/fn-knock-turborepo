import { computed, nextTick, ref, type ComputedRef, type Ref } from "vue";
import { ScanAPI } from "@/lib/api";
import type {
  DiscoveredServiceInfo,
  ScanDiscoverPollEvent,
  ScanDiscoverResponse,
} from "@/lib/api";
import type { ProxyMapping } from "@/types";
import { useAsyncAction } from "@admin-shared/composables/useAsyncAction";
import { useDiscoverServicesSelection } from "@admin-shared/composables/useDiscoverServicesSelection";
import { toast } from "@admin-shared/utils/toast";
import { buildProxyMapping } from "@admin-shared/utils/proxyMapping";
import { persistProxyMappings } from "@admin-shared/utils/persistProxyMappings";
import {
  createReverseProxyMessages,
  showReverseProxyActionError,
  showReverseProxyDuplicateItemsError,
} from "@admin-shared/utils/reverseProxyFeedback";
import { validateBatchMappingDuplicates } from "@admin-shared/utils/validateProxyMappingDuplicates";

type ReverseProxyMessages = ReturnType<typeof createReverseProxyMessages>;
type Translate = (key: string, params?: Record<string, unknown>) => string;
type DiscoverTargetsSettingsHandle = {
  ensureSaved: () => Promise<string[]> | undefined;
  loadTargets: () => Promise<void> | undefined;
};
type RunAsyncAction = <T>(
  action: () => Promise<T>,
  hooks?: { onFinally?: () => void },
) => Promise<T | undefined>;

const isDiscoverAbortError = (error: unknown): boolean =>
  error instanceof DOMException
    ? error.name === "AbortError"
    : error instanceof Error && error.name === "AbortError";

const createEmptyDiscoverResponse = (
  patch: Partial<ScanDiscoverResponse> = {},
): ScanDiscoverResponse => ({
  host: patch.host || "",
  totalPortsScanned: patch.totalPortsScanned || 0,
  foundServices: patch.foundServices || 0,
  scannedHosts: patch.scannedHosts,
  scanHostCount: patch.scanHostCount,
  scanScope: patch.scanScope,
  scanCidrs: patch.scanCidrs,
  intensityMode: patch.intensityMode,
  intensityLevel: patch.intensityLevel,
  recommendedLevel: patch.recommendedLevel,
  configuredConcurrency: patch.configuredConcurrency,
  effectiveConcurrency: patch.effectiveConcurrency,
  services: [],
});

const cloneDiscoveredService = (
  service: DiscoveredServiceInfo,
): DiscoveredServiceInfo => ({
  ...service,
  detail: {
    ...service.detail,
    rule: { ...service.detail.rule },
  },
});

export const useReverseProxyDiscoverFlow = ({
  allMappings,
  currentHostname,
  currentPage,
  discoverTargetsSettingsRef,
  messages,
  runSaveAction,
  saveDefaultRoute,
  saveProxyMappings,
  searchQuery,
  translate,
}: {
  allMappings: ComputedRef<ProxyMapping[]>;
  currentHostname: string;
  currentPage: Ref<number>;
  discoverTargetsSettingsRef: Ref<DiscoverTargetsSettingsHandle | null>;
  messages: ReverseProxyMessages;
  runSaveAction: RunAsyncAction;
  saveDefaultRoute: (path: string) => Promise<void>;
  saveProxyMappings: (mappings: ProxyMapping[]) => Promise<void>;
  searchQuery: Ref<string>;
  translate: Translate;
}) => {
  const isDiscoverSettingsOpen = ref(false);
  const discoverAbortController = ref<AbortController | null>(null);
  const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction(
    {
      onError: (error) => {
        if (isDiscoverAbortError(error)) return;
        showReverseProxyActionError(
          messages.scanFailed,
          error,
          messages.unknownError,
        );
      },
    },
  );
  const {
    open: isDiscoverDialogOpen,
    discoveredData,
    selectedServices,
    isAllSelected,
    isSelectionValid: isDiscoverSelectionValid,
    setAllSelected,
    resetSelection,
    setDiscoveredData,
    openDialog: openDiscoverDialogState,
    closeDialog: closeDiscoverDialog,
  } = useDiscoverServicesSelection<DiscoveredServiceInfo, ScanDiscoverResponse>(
    {
      getPath: (service) => service.detail.rule.path,
    },
  );

  const showDiscoverHostColumn = computed(() => {
    const hosts = new Set(
      (discoveredData.value?.services || [])
        .map((service) => service.host?.trim())
        .filter(Boolean),
    );
    return hosts.size > 1;
  });
  const resolveDiscoveredServiceHost = (service: DiscoveredServiceInfo) =>
    service.host?.trim() ||
    discoveredData.value?.host?.trim() ||
    currentHostname;

  const upsertDiscoveredService = (service: DiscoveredServiceInfo) => {
    const current = discoveredData.value || createEmptyDiscoverResponse();
    const nextService = cloneDiscoveredService(service);
    const serviceKey =
      nextService.serviceKey ||
      `${nextService.host?.trim() || current.host}:${nextService.port}`;
    const nextServices = [...current.services];
    const existingIndex = nextServices.findIndex((item) => {
      const itemKey =
        item.serviceKey || `${item.host?.trim() || current.host}:${item.port}`;
      return itemKey === serviceKey;
    });

    if (existingIndex >= 0) {
      const previous = nextServices[existingIndex]!;
      nextServices[existingIndex] = nextService;
      const selectedIndex = selectedServices.value.indexOf(previous);
      if (selectedIndex >= 0) {
        selectedServices.value[selectedIndex] = nextService;
      }
    } else {
      nextServices.push(nextService);
      if (nextService.detail.rule.path?.trim()) {
        selectedServices.value.push(nextService);
      }
    }

    setDiscoveredData({
      ...current,
      foundServices: nextServices.length,
      services: nextServices,
    });
  };

  const applyDiscoverPollEvent = (event: ScanDiscoverPollEvent) => {
    if (event.type === "meta") {
      setDiscoveredData(createEmptyDiscoverResponse(event.data));
      return;
    }

    if (event.type === "progress") return;

    if (event.type === "service") {
      upsertDiscoveredService(event.data.service);
      return;
    }

    if (event.type === "done") {
      const current = discoveredData.value;
      if (!current) {
        setDiscoveredData(event.data);
        selectedServices.value = event.data.services.filter((service) =>
          Boolean(service.detail.rule.path?.trim()),
        );
        return;
      }

      setDiscoveredData({
        ...current,
        ...event.data,
        foundServices: current.services.length,
        services: current.services,
      });
    }
  };

  const onToggleAllDiscoverSelect = (event: Event) => {
    setAllSelected((event.target as HTMLInputElement).checked);
  };

  const stopDiscoverScan = () => {
    discoverAbortController.value?.abort();
    discoverAbortController.value = null;
  };

  const dismissDiscoverDialog = () => {
    stopDiscoverScan();
    closeDiscoverDialog(true);
    isDiscoverSettingsOpen.value = false;
  };

  const handleDiscoverDialogOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) {
      dismissDiscoverDialog();
    }
  };

  const triggerScan = async () => {
    let targetCidrs: string[];
    try {
      await nextTick();
      const selectedCidrs =
        await discoverTargetsSettingsRef.value?.ensureSaved();
      if (!selectedCidrs || selectedCidrs.length === 0) return;
      targetCidrs = selectedCidrs;
    } catch {
      return;
    }

    resetSelection();
    discoverAbortController.value?.abort();
    const abortController = new AbortController();
    discoverAbortController.value = abortController;
    await runDiscoverServices(
      () =>
        ScanAPI.discoverPolling(
          { target_cidrs: targetCidrs },
          {
            signal: abortController.signal,
            onEvent: applyDiscoverPollEvent,
          },
        ),
      {
        onFinally: () => {
          if (discoverAbortController.value === abortController) {
            discoverAbortController.value = null;
          }
        },
      },
    );
  };

  const openDiscoverDialog = () => {
    openDiscoverDialogState();
    if (!discoveredData.value) {
      void nextTick().then(() => triggerScan());
    }
  };

  const toggleDiscoverSettings = async () => {
    isDiscoverSettingsOpen.value = !isDiscoverSettingsOpen.value;
    if (isDiscoverSettingsOpen.value) {
      await nextTick();
      void discoverTargetsSettingsRef.value?.loadTargets();
    }
  };

  const saveDiscoveredServices = async () => {
    if (!isDiscoverSelectionValid.value || !discoveredData.value) return;
    const candidates = selectedServices.value.map((service) => ({
      path: service.detail.rule.path?.trim() || "",
      target:
        `http://${resolveDiscoveredServiceHost(service)}:${service.port}/`.trim(),
    }));
    const { duplicatePaths, duplicateTargets } = validateBatchMappingDuplicates(
      allMappings.value,
      candidates,
    );

    if (duplicatePaths.length > 0) {
      showReverseProxyDuplicateItemsError(
        messages.duplicateItems(
          translate("admin.reverseProxy.duplicatePathLabel"),
          duplicatePaths,
        ),
      );
      return;
    }
    if (duplicateTargets.length > 0) {
      showReverseProxyDuplicateItemsError(
        messages.duplicateItems(
          translate("admin.reverseProxy.duplicateTargetLabel"),
          duplicateTargets,
        ),
      );
      return;
    }

    stopDiscoverScan();
    await runSaveAction(async () => {
      const nextMappings = [...allMappings.value];
      let defaultRoutePath: string | null = null;

      for (const service of selectedServices.value) {
        const rule = service.detail.rule;
        const mapping = buildProxyMapping({
          path: rule.path,
          target: `http://${resolveDiscoveredServiceHost(service)}:${service.port}/`,
          rewrite_html: rule.rewrite_html,
          use_auth: rule.use_auth,
          use_root_mode: rule.use_root_mode,
          strip_path: rule.strip_path,
        });
        nextMappings.push(mapping);

        if (service.detail.isDefault) {
          defaultRoutePath = mapping.path;
        }
      }

      await persistProxyMappings(
        nextMappings,
        {
          saveMappings: saveProxyMappings,
          saveDefaultRoute,
          resetPage: () => {
            currentPage.value = 1;
          },
          resetSearch: () => {
            searchQuery.value = "";
          },
        },
        {
          defaultRoutePath,
          resetPage: true,
          onAfterPersist: () => {
            toast.success(
              messages.discoverSaveSuccess(selectedServices.value.length),
            );
            dismissDiscoverDialog();
          },
        },
      );
    });
  };

  return {
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
  };
};
