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
import { ScanAPI } from "@/lib/api";
import type { HostMapping } from "@/types";
import {
  buildDiscoveredHostResponse,
  buildDiscoveredServiceMappings,
  collectDuplicateValues,
  composeHostFromSubdomain,
  type DiscoveredHostResponse,
  type DiscoveredHostService,
  type TranslationParams,
} from "./model";

type RunAsyncAction = <T>(
  action: () => Promise<T>,
) => Promise<T | undefined>;

type DiscoverDialogHandle = {
  ensureSaved: () => Promise<string[]> | undefined;
  loadTargets: () => Promise<void> | undefined;
};

export const useSubdomainDiscoverFlow = ({
  allMappings,
  canManageNewMappings,
  existingMappingPorts,
  runSaveMappings,
  savedRootDomain,
  saveHostMappings,
  translate,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  canManageNewMappings: ComputedRef<boolean>;
  existingMappingPorts: ComputedRef<Set<number>>;
  runSaveMappings: RunAsyncAction;
  savedRootDomain: ComputedRef<string>;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const discoverDialogRef = ref<DiscoverDialogHandle | null>(null);
  const setDiscoverDialogRef = (
    instance: Element | ComponentPublicInstance | null,
  ) => {
    discoverDialogRef.value = instance as DiscoverDialogHandle | null;
  };
  const isDiscoverSettingsOpen = ref(false);

  const { isPending: isDiscovering, run: runDiscoverServices } =
    useAsyncAction({
      onError: (error) => {
        toast.error(translate("admin.subdomainProxy.discoverFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.subdomainProxy.discoverServicesFailed"),
          ),
        });
      },
    });

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

  const dismissDiscoverDialog = () => {
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
    let targetCidrs: string[] | undefined;
    try {
      await nextTick();
      targetCidrs = await discoverDialogRef.value?.ensureSaved();
    } catch {
      return;
    }

    resetSelection();
    await runDiscoverServices(
      () => ScanAPI.discover({ target_cidrs: targetCidrs }),
      {
        onSuccess: (data) => {
          const nextData = buildDiscoveredHostResponse(
            data,
            existingMappingPorts.value,
          );
          setDiscoveredData(nextData);
          selectedServices.value = nextData.services.filter((service) =>
            Boolean(service.suggestedSubdomain.trim()),
          );
        },
      },
    );
  };

  const openDiscoverDialog = () => {
    if (!canManageNewMappings.value) {
      toast.error(translate("admin.subdomainProxy.cannotDiscover"), {
        description: !savedRootDomain.value
          ? translate("admin.subdomainProxy.saveRootFirst")
          : translate("admin.subdomainProxy.rootDirtyDiscover"),
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

    await runSaveMappings(async () => {
      const next = [
        ...allMappings.value,
        ...buildDiscoveredServiceMappings({
          fallbackHost: discoveredData.value?.host,
          rootDomain: savedRootDomain.value,
          services: selectedServices.value,
        }),
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
    setAllSelected,
    showDiscoverHostColumn,
    toggleDiscoverSettings,
    triggerScan,
  };
};
