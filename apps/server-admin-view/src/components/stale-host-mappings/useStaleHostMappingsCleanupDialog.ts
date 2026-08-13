import { computed, nextTick, reactive } from "vue";
import { useI18n } from "vue-i18n";
import { useStaleHostMappingsCleanup } from "@/composables/useStaleHostMappingsCleanup";
import type {
  HostMappingProbeResult,
  HostMappingProbeStatus,
} from "@/lib/api/scan";
import type { HostMapping } from "@/types";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";

export type StaleHostMappingsCleanupDialogOptions = {
  mappings: () => HostMapping[];
  saveMappings: (mappings: HostMapping[]) => Promise<unknown>;
  isAuthServiceTarget: (target: string) => boolean;
  onCleaned: (count: number) => void;
};

export const useStaleHostMappingsCleanupDialog = (
  options: StaleHostMappingsCleanupDialogOptions,
) => {
  const { t } = useI18n();
  const mappingsSource = computed(options.mappings);
  const mappingTitleByHost = computed(() => {
    const titles = new Map<string, string>();
    for (const mapping of mappingsSource.value) {
      titles.set(
        mapping.host.trim().toLowerCase(),
        mapping.title_override.trim() ||
          mapping.title.trim() ||
          t("admin.subdomainProxy.notFetched"),
      );
    }
    return titles;
  });
  const resource = useStaleHostMappingsCleanup({
    mappings: mappingsSource,
    saveMappings: options.saveMappings,
    isAuthServiceTarget: options.isAuthServiceTarget,
  });
  const visibleResults = computed(() =>
    resource.results.value.filter((result) => result.status !== "online"),
  );

  const handleProbe = async () => {
    if (resource.probeableMappings.value.length === 0) {
      resource.results.value = [];
      return;
    }
    try {
      await resource.probe();
    } catch (error) {
      toast.error(t("admin.subdomainProxy.staleCleanupProbeFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.staleCleanupProbeFailedDescription"),
        ),
      });
    }
  };
  const openCleanupDialog = async () => {
    resource.openDialog();
    await nextTick();
    await handleProbe();
  };
  const handleOpenChange = (nextOpen: boolean) => {
    if (!nextOpen) resource.closeDialog();
  };
  const handleCleanSelected = async () => {
    try {
      const cleanedCount = await resource.cleanSelected();
      if (cleanedCount <= 0) return;
      toast.success(t("admin.subdomainProxy.staleCleanupCleaned"), {
        description: t(
          "admin.subdomainProxy.staleCleanupCleanedDescription",
          { count: cleanedCount },
        ),
      });
      options.onCleaned(cleanedCount);
      resource.closeDialog();
    } catch (error) {
      toast.error(t("admin.subdomainProxy.staleCleanupCleanFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.subdomainProxy.staleCleanupCleanFailedDescription"),
        ),
      });
    }
  };
  const handleToggleAllStale = (event: Event) => {
    resource.setAllStaleSelected(
      (event.target as HTMLInputElement).checked,
    );
  };
  const handleToggleHost = (host: string, event: Event) => {
    resource.setHostSelected(
      host,
      (event.target as HTMLInputElement).checked,
    );
  };
  const getMappingTitle = (host: string) =>
    mappingTitleByHost.value.get(host.trim().toLowerCase()) ||
    t("admin.subdomainProxy.notFetched");
  const getStatusLabel = (result: HostMappingProbeResult) => {
    if (result.status === "online" && result.httpStatus) {
      return `${t("admin.subdomainProxy.staleCleanupStatus.online")} ${
        result.httpStatus
      }`;
    }
    return t(`admin.subdomainProxy.staleCleanupStatus.${result.status}`);
  };
  const getStatusBadgeVariant = (status: HostMappingProbeStatus) =>
    status === "stale" ? "destructive" : "secondary";
  const getStatusBadgeClass = (status: HostMappingProbeStatus) => {
    if (status === "online") {
      return "bg-emerald-500/10 text-emerald-700";
    }
    if (status === "unsupported") {
      return "bg-muted text-muted-foreground";
    }
    return "";
  };

  return reactive({
    closeDialog: resource.closeDialog,
    getMappingTitle,
    getStatusBadgeClass,
    getStatusBadgeVariant,
    getStatusLabel,
    handleCleanSelected,
    handleOpenChange,
    handleProbe,
    handleToggleAllStale,
    handleToggleHost,
    isAllStaleSelected: resource.isAllStaleSelected,
    isCleaning: resource.isCleaning,
    isHostSelected: resource.isHostSelected,
    isOpen: resource.open,
    isProbing: resource.isProbing,
    openCleanupDialog,
    probeableMappings: resource.probeableMappings,
    results: resource.results,
    selectedCount: resource.selectedCount,
    staleResults: resource.staleResults,
    visibleResults,
  });
};

export type StaleHostMappingsCleanupDialogModel = ReturnType<
  typeof useStaleHostMappingsCleanupDialog
>;
