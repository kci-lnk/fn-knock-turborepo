import type { ComputedRef, Ref } from "vue";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { downloadBlob } from "@admin-shared/utils/downloadBlob";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import type { HostMapping } from "@/types";
import {
  buildBookmarkExportFilename,
  hasSameMappingOrder,
  mergeFilteredMappingsOrder,
} from "./model";

type AsyncActionRun = <T>(
  action: () => Promise<T>,
  hooks?: {
    onSuccess?: (result: T) => void | Promise<void>;
  },
) => Promise<T | undefined>;

type SyncRoutesResult = {
  success: boolean;
  message?: string;
  data?: {
    synced_host_rules?: number;
    synced_rules?: number;
  };
};

type RefreshTitlesSummary = {
  failed: number;
  skipped: number;
  updated: number;
};

type Translate = (key: string, params?: Record<string, unknown>) => string;

export const useSubdomainMappingListActions = ({
  allMappings,
  downloadBookmarks,
  draggableVisibleMappings,
  filteredMappings,
  formatHostWithAccessEntryPort,
  isAuthServiceTarget,
  isDefaultDomainAvailable,
  isSavingMappings,
  navigateToGatewayLocations,
  refreshAllHostMappingTitles,
  resetFaviconErrors,
  runSaveMappings,
  saveHostMappings,
  savedRootDomain,
  syncDraggableVisibleMappings,
  syncRoutesApi,
  translate,
  visibleMappings,
}: {
  allMappings: ComputedRef<HostMapping[]>;
  downloadBookmarks: () => Promise<Blob>;
  draggableVisibleMappings: Ref<HostMapping[]>;
  filteredMappings: ComputedRef<HostMapping[]>;
  formatHostWithAccessEntryPort: (host: string) => string;
  isAuthServiceTarget: (target: string) => boolean;
  isDefaultDomainAvailable: ComputedRef<boolean>;
  isSavingMappings: Ref<boolean>;
  navigateToGatewayLocations: (host: string) => void;
  refreshAllHostMappingTitles: () => Promise<RefreshTitlesSummary>;
  resetFaviconErrors: () => void;
  runSaveMappings: AsyncActionRun;
  saveHostMappings: (mappings: HostMapping[]) => Promise<unknown>;
  savedRootDomain: ComputedRef<string>;
  syncDraggableVisibleMappings: () => void;
  syncRoutesApi: () => Promise<SyncRoutesResult>;
  translate: Translate;
  visibleMappings: ComputedRef<HostMapping[]>;
}) => {
  const { isPending: isSyncing, run: runSyncRoutes } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.subdomainProxy.syncFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.subdomainProxy.syncGatewayFailed"),
        ),
      });
    },
  });
  const { isPending: isRefreshingTitles, run: runRefreshTitles } =
    useAsyncAction({
      onError: (error) => {
        toast.error(translate("admin.subdomainProxy.refreshFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.subdomainProxy.refreshAllTitlesFailed"),
          ),
        });
      },
    });
  const { isPending: isExportingBookmarks, run: runExportBookmarks } =
    useAsyncAction({
      onError: (error) => {
        toast.error(translate("admin.subdomainProxy.exportFailed"), {
          description: extractErrorMessage(
            error,
            translate("admin.subdomainProxy.exportBookmarksFailed"),
          ),
        });
      },
    });
  const saveMappingOrder = async () => {
    const next = mergeFilteredMappingsOrder({
      allMappings: allMappings.value,
      filteredMappings: filteredMappings.value,
      isPinnedMapping: (mapping) => isAuthServiceTarget(mapping.target),
      nextFiltered: draggableVisibleMappings.value,
      visibleMappings: visibleMappings.value,
    });
    if (hasSameMappingOrder(next, allMappings.value)) {
      syncDraggableVisibleMappings();
      return;
    }

    const saved = await runSaveMappings(async () => {
      await saveHostMappings(next);
      toast.success(translate("admin.subdomainProxy.orderUpdated"));
      return true;
    });

    if (saved !== true) {
      syncDraggableVisibleMappings();
    }
  };

  const copyMappingHost = async (mapping: HostMapping) => {
    const host = formatHostWithAccessEntryPort(mapping.host);
    if (!host) return;

    try {
      const result = await copyTextToClipboard(host);
      if (result.verified) {
        toast.success(translate("admin.subdomainProxy.hostCopied"), {
          description: host,
        });
        return;
      }

      toast.info(translate("admin.subdomainProxy.copyAttempted"), {
        description: host,
      });
    } catch {
      toast.error(translate("admin.subdomainProxy.copyFailed"), {
        description: translate("admin.subdomainProxy.copyRestricted"),
      });
    }
  };

  const openGatewayLocations = (host: string) => {
    navigateToGatewayLocations(host);
  };

  const setDefaultMapping = async (mapping: HostMapping) => {
    if (
      isSavingMappings.value ||
      !isDefaultDomainAvailable.value ||
      isAuthServiceTarget(mapping.target)
    ) {
      return;
    }

    await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((item) => ({
          ...item,
          is_default: item.host === mapping.host,
        })),
      );
      toast.success(translate("admin.subdomainProxy.defaultDomainSet"), {
        description: formatHostWithAccessEntryPort(mapping.host),
      });
      return true;
    });
  };

  const clearDefaultMapping = async (mapping: HostMapping) => {
    if (isSavingMappings.value || !isDefaultDomainAvailable.value) {
      return;
    }

    await runSaveMappings(async () => {
      await saveHostMappings(
        allMappings.value.map((item) =>
          item.is_default ? { ...item, is_default: false } : item,
        ),
      );
      toast.success(translate("admin.subdomainProxy.defaultDomainCleared"), {
        description: formatHostWithAccessEntryPort(mapping.host),
      });
      return true;
    });
  };

  const syncRoutes = async () => {
    await runSyncRoutes(() => syncRoutesApi(), {
      onSuccess: (result) => {
        if (result.success) {
          toast.success(translate("admin.subdomainProxy.syncedGateway"), {
            description: translate(
              "admin.subdomainProxy.syncedGatewayDescription",
              {
                hostRules: result.data?.synced_host_rules ?? 0,
                pathRules: result.data?.synced_rules ?? 0,
              },
            ),
          });
          return;
        }
        toast.error(translate("admin.subdomainProxy.syncFailed"), {
          description:
            result.message || translate("admin.subdomainProxy.syncNoSuccess"),
        });
      },
    });
  };

  const refreshAllTitles = async () => {
    await runRefreshTitles(() => refreshAllHostMappingTitles(), {
      onSuccess: (summary) => {
        toast.success(translate("admin.subdomainProxy.titlesRefreshDone"), {
          description: translate(
            "admin.subdomainProxy.titlesRefreshDescription",
            {
              failed: summary.failed,
              skipped: summary.skipped,
              updated: summary.updated,
            },
          ),
        });
        resetFaviconErrors();
      },
    });
  };

  const exportBookmarks = async () => {
    await runExportBookmarks(() => downloadBookmarks(), {
      onSuccess: (blob) => {
        downloadBlob(blob, buildBookmarkExportFilename(savedRootDomain.value));
        toast.success(translate("admin.subdomainProxy.bookmarksExported"), {
          description: translate(
            "admin.subdomainProxy.bookmarksExportDescription",
            {
              count: visibleMappings.value.length,
            },
          ),
        });
      },
    });
  };

  return {
    clearDefaultMapping,
    copyMappingHost,
    exportBookmarks,
    isExportingBookmarks,
    isRefreshingTitles,
    isSyncing,
    openGatewayLocations,
    refreshAllTitles,
    saveMappingOrder,
    setDefaultMapping,
    syncRoutes,
  };
};
