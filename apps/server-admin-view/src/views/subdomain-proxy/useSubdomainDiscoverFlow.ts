import {
  computed,
  nextTick,
  ref,
  type ComponentPublicInstance,
  type ComputedRef,
} from "vue";
import { useDiscoverServicesSelection } from "@admin-shared/composables/useDiscoverServicesSelection";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ScanAPI } from "@/lib/api/scan";
import {
  type DiscoveredServiceInfo,
  type ScanDiscoverPollEvent,
  type ScanDiscoverProgress,
  type ScanDiscoverResponse,
} from "@/lib/api/scan";
import type { HostMapping } from "@/types";
import {
  buildDiscoveredHostResponse,
  buildDiscoveredServiceMappings,
  collectDuplicateValues,
  composeHostFromSubdomain,
  resolveDiscoveredServiceHost,
  type DiscoveredHostResponse,
  type DiscoveredHostService,
  type TranslationParams,
} from "./model";

type RunAsyncAction = <T>(action: () => Promise<T>) => Promise<T | undefined>;

const isDiscoverAbortError = (error: unknown): boolean =>
  error instanceof DOMException
    ? error.name === "AbortError"
    : error instanceof Error && error.name === "AbortError";

type DiscoverDialogHandle = {
  ensureSaved: () => Promise<string[]> | undefined;
  loadTargets: () => Promise<void> | undefined;
};

export const useSubdomainDiscoverFlow = ({
  allMappings,
  canManageNewMappings,
  existingMappingTargets,
  runSaveMappings,
  rootDomainValidationMessage,
  savedRootDomain,
  saveHostMappings,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  canManageNewMappings: ComputedRef<boolean>;
  existingMappingTargets: ComputedRef<Set<string>>;
  runSaveMappings: RunAsyncAction;
  rootDomainValidationMessage: ComputedRef<string>;
  savedRootDomain: ComputedRef<string>;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const discoverDialogRef = ref<DiscoverDialogHandle | null>(null);
  const discoverGroupId = ref<string | null>(null);
  const discoverAbortController = ref<AbortController | null>(null);
  const discoverProgress = ref<ScanDiscoverProgress | null>(null);
  const setDiscoverDialogRef = (
    instance: Element | ComponentPublicInstance | null,
  ) => {
    discoverDialogRef.value = instance as DiscoverDialogHandle | null;
  };
  const isDiscoverSettingsOpen = ref(false);

  const { isPending: isDiscovering, run: runDiscoverServices } = useAsyncAction(
    {
      onError: (error) => {
        if (isDiscoverAbortError(error)) return;
        toast.error(translate("admin.subdomainProxy.discoverFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.subdomainProxy.discoverServicesFailed"),
          ),
        });
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
  } = useDiscoverServicesSelection<
    DiscoveredHostService,
    DiscoveredHostResponse
  >({
    getPath: (service) => service.suggestedSubdomain,
  });

  const showDiscoverHostColumn = computed(() => {
    const hosts = new Set(
      (discoveredData.value?.services || [])
        .map((service) => service.host?.trim())
        .filter(Boolean),
    );
    return hosts.size > 1;
  });

  const createEmptyDiscoveredResponse = (
    patch: Partial<ScanDiscoverResponse> = {},
  ): DiscoveredHostResponse => ({
    host: patch.host || "",
    totalPortsScanned: patch.totalPortsScanned || 0,
    foundServices: patch.foundServices || 0,
    scannedHosts: patch.scannedHosts ?? 0,
    scanHostCount: patch.scanHostCount ?? 0,
    scanScope: patch.scanScope ?? null,
    scanCidrs: patch.scanCidrs ?? [],
    intensityMode: patch.intensityMode ?? "auto",
    intensityLevel: patch.intensityLevel ?? "low",
    recommendedLevel: patch.recommendedLevel ?? "low",
    configuredConcurrency: patch.configuredConcurrency ?? 0,
    effectiveConcurrency: patch.effectiveConcurrency ?? 0,
    services: [],
  });

  const upsertDiscoveredService = (service: DiscoveredServiceInfo) => {
    const current =
      discoveredData.value || createEmptyDiscoveredResponse({ services: [] });
    const transformed = buildDiscoveredHostResponse(
      {
        ...current,
        services: [service],
      },
      existingMappingTargets.value,
    ).services[0];
    if (!transformed) return;

    const serviceKey =
      service.serviceKey ||
      `${resolveDiscoveredServiceHost(service, current.host)}:${service.port}`;
    const nextServices = [...current.services];
    const existingIndex = nextServices.findIndex((item) => {
      const itemKey =
        item.serviceKey ||
        `${resolveDiscoveredServiceHost(item, current.host)}:${item.port}`;
      return itemKey === serviceKey;
    });

    if (existingIndex >= 0) {
      const previous = nextServices[existingIndex]!;
      nextServices[existingIndex] = transformed;
      const selectedIndex = selectedServices.value.indexOf(previous);
      if (selectedIndex >= 0) {
        selectedServices.value[selectedIndex] = transformed;
      }
    } else {
      nextServices.push(transformed);
      if (transformed.suggestedSubdomain.trim()) {
        selectedServices.value.push(transformed);
      }
    }

    setDiscoveredData({
      ...current,
      foundServices: nextServices.length,
      services: nextServices,
    });
  };

  const applyDiscoverEvent = (event: ScanDiscoverPollEvent) => {
    if (event.type === "meta") {
      setDiscoveredData(createEmptyDiscoveredResponse(event.data));
      return;
    }

    if (event.type === "progress") {
      discoverProgress.value = event.data;
      return;
    }

    if (event.type === "service") {
      upsertDiscoveredService(event.data.service);
      return;
    }

    if (event.type === "done") {
      const current = discoveredData.value;
      if (!current) {
        setDiscoveredData(
          buildDiscoveredHostResponse(event.data, existingMappingTargets.value),
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

  const dismissDiscoverDialog = () => {
    stopDiscoverScan();
    setDiscoveredData(null);
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
      const selectedCidrs = await discoverDialogRef.value?.ensureSaved();
      if (!selectedCidrs || selectedCidrs.length === 0) return;
      targetCidrs = selectedCidrs;
    } catch {
      return;
    }

    resetSelection();
    discoverProgress.value = null;
    discoverAbortController.value?.abort();
    const abortController = new AbortController();
    discoverAbortController.value = abortController;
    await runDiscoverServices(
      () =>
        ScanAPI.discoverPolling(
          { target_cidrs: targetCidrs },
          {
            signal: abortController.signal,
            onEvent: applyDiscoverEvent,
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

  const stopDiscoverScan = () => {
    discoverAbortController.value?.abort();
    discoverAbortController.value = null;
  };

  const openDiscoverDialog = () => {
    if (!canManageNewMappings.value) {
      toast.error(translate("admin.subdomainProxy.cannotDiscover"), {
        description:
          rootDomainValidationMessage.value ||
          (!savedRootDomain.value
            ? translate("admin.subdomainProxy.saveRootFirst")
            : translate("admin.subdomainProxy.rootDirtyDiscover")),
      });
      return;
    }

    openDiscoverDialogState();
    if (!discoveredData.value) {
      void nextTick().then(() => triggerScan());
    }
  };

  const toggleDiscoverSettings = async () => {
    isDiscoverSettingsOpen.value = !isDiscoverSettingsOpen.value;
    if (isDiscoverSettingsOpen.value) {
      await nextTick();
      void discoverDialogRef.value?.loadTargets();
    }
  };

  const saveDiscoveredServices = async () => {
    if (
      !isDiscoverSelectionValid.value ||
      !savedRootDomain.value ||
      !discoveredData.value
    ) {
      return;
    }

    const candidateHosts = selectedServices.value.map((service) =>
      composeHostFromSubdomain(
        service.suggestedSubdomain,
        savedRootDomain.value,
      ),
    );
    const existingHostSet = new Set(allMappings.value.map((item) => item.host));
    const duplicateHosts = [
      ...new Set([
        ...candidateHosts.filter((host) => existingHostSet.has(host)),
        ...collectDuplicateValues(candidateHosts),
      ]),
    ];

    if (duplicateHosts.length > 0) {
      toast.error(translate("admin.subdomainProxy.duplicateDiscoverHosts"), {
        description: duplicateHosts.join(", "),
      });
      return;
    }

    stopDiscoverScan();
    await runSaveMappings(async () => {
      const next = [
        ...allMappings.value,
        ...buildDiscoveredServiceMappings({
          fallbackHost: discoveredData.value?.host,
          rootDomain: savedRootDomain.value,
          services: selectedServices.value,
        }).map((mapping) => ({
          ...mapping,
          group_id: discoverGroupId.value,
        })),
      ];

      await saveHostMappings(next);
      toast.success(
        translate("admin.subdomainProxy.addedMappings", {
          count: selectedServices.value.length,
        }),
      );
      dismissDiscoverDialog();
    });
  };

  return {
    discoveredData,
    discoverGroupId,
    discoverProgress,
    dismissDiscoverDialog,
    handleDiscoverDialogOpenChange,
    isAllSelected,
    isDiscoverDialogOpen,
    isDiscoverSelectionValid,
    isDiscoverSettingsOpen,
    isDiscovering,
    openDiscoverDialog,
    saveDiscoveredServices,
    selectedServices,
    setDiscoverDialogRef,
    stopDiscoverScan,
    setAllSelected,
    showDiscoverHostColumn,
    toggleDiscoverSettings,
    triggerScan,
  };
};
