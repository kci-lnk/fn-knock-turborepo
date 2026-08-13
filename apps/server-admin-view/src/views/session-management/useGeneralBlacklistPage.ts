import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  GeneralBlacklistAPI,
  type GeneralBlacklistRecord,
  type GeneralBlacklistSource,
} from "@/lib/api/security";
import { useIpLocationBatch } from "../../composables/useIpLocationBatch";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import { toast } from "@admin-shared/utils/toast";

export function useGeneralBlacklistPage() {
  const { t } = useI18n();
  const addDialogOpen = ref(false);
  const addIpsText = ref("");
  const addComment = ref("");

  const {
    items: records,
    total: totalRecords,
    loading,
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    selectedKeys: selectedIps,
    isAllSelected,
    fetchList: fetchBlacklist,
    handleSearch,
    handlePageChange,
    handleLimitChange,
    toggleSelect,
    clearSelection,
  } = usePagedSelectionList<GeneralBlacklistRecord, string>({
    fetchPage: async ({ page, limit: pageLimit, query }) => {
      const data = await GeneralBlacklistAPI.getList(page, pageLimit, query);
      return {
        items: data.items || [],
        total: data.total || 0,
      };
    },
    getKey: (record) => record.ip,
    onError: (error) => {
      toast.error(t("admin.sessions.generalBlacklist.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.generalBlacklist.loadFailed"),
        ),
      });
    },
  });

  const { trackIps, getSnapshot } = useIpLocationBatch();
  const { isPending: isAdding, run: runAddAction } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessions.generalBlacklist.addFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.generalBlacklist.addFailed"),
        ),
      });
    },
  });
  const { isPending: isDeleting, run: runDeleteAction } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.sessions.generalBlacklist.deleteFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.sessions.generalBlacklist.deleteFailed"),
        ),
      });
    },
  });
  const showTableSkeleton = useDelayedLoading(
    () => loading.value && records.value.length === 0,
  );
  const parsedAddIps = computed(() =>
    Array.from(
      new Set(
        addIpsText.value
          .split(/[\s,;]+/)
          .map((item) => item.trim())
          .filter(Boolean),
      ),
    ),
  );

  const getSourceLabel = (source?: string) => {
    switch (source) {
      case "request_log":
        return t("admin.sessions.generalBlacklist.sources.requestLog");
      case "active_ip":
        return t("admin.sessions.generalBlacklist.sources.activeIp");
      case "waf_log":
        return t("admin.sessions.generalBlacklist.sources.wafLog");
      default:
        return t("admin.sessions.generalBlacklist.sources.manual");
    }
  };

  const getSourceVariant = (
    source?: string,
  ): "default" | "secondary" | "outline" | "destructive" => {
    if (source === "request_log") return "secondary";
    if (source === "active_ip") return "outline";
    if (source === "waf_log") return "destructive";
    return "default";
  };

  const getLocationText = (ip: string) => {
    const snapshot = getSnapshot(ip);
    if (snapshot?.location) return snapshot.location;
    if (snapshot?.status === "queued" || snapshot?.status === "processing") {
      return t("admin.hostActiveIps.resolving");
    }
    if (snapshot?.status === "skipped") {
      return t("admin.hostActiveIps.privateAddress");
    }
    return t("admin.hostActiveIps.unavailable");
  };

  const addBlacklist = async (
    ips: string[],
    source: GeneralBlacklistSource,
    comment?: string,
  ) => {
    if (ips.length === 0) return;
    await runAddAction(() => GeneralBlacklistAPI.add(ips, source, comment), {
      onSuccess: async (result) => {
        toast.success(t("admin.sessions.generalBlacklist.addSuccess"), {
          description: t("admin.sessions.generalBlacklist.addSuccessDetail", {
            added: result?.added ?? 0,
            updated: result?.updated ?? 0,
          }),
        });
        addDialogOpen.value = false;
        addIpsText.value = "";
        addComment.value = "";
        await fetchBlacklist();
      },
    });
  };

  const addManualBlacklist = async () => {
    await addBlacklist(parsedAddIps.value, "manual", addComment.value.trim());
  };

  const deleteBlacklist = async (ips: string[]) => {
    if (ips.length === 0) return;
    await runDeleteAction(() => GeneralBlacklistAPI.delete(ips), {
      onSuccess: async (result) => {
        toast.success(t("admin.sessions.generalBlacklist.deleteSuccess"), {
          description: t(
            "admin.sessions.generalBlacklist.deleteSuccessDetail",
            { removed: result?.removed ?? 0 },
          ),
        });
        clearSelection();
        await fetchBlacklist();
      },
    });
  };

  const deleteOne = async (ip: string) => {
    await runDeleteAction(() => GeneralBlacklistAPI.deleteByIp(ip), {
      onSuccess: async () => {
        toast.success(t("admin.sessions.generalBlacklist.deleteSuccess"));
        selectedIps.value.delete(ip);
        selectedIps.value = new Set(selectedIps.value);
        await fetchBlacklist();
      },
    });
  };

  watch(
    records,
    (items) => {
      trackIps(items.map((record) => record.ip));
    },
    { immediate: true },
  );
  onMounted(() => {
    void fetchBlacklist();
  });

  return {
    addComment,
    addDialogOpen,
    addIpsText,
    addManualBlacklist,
    currentPage,
    deleteBlacklist,
    deleteOne,
    fetchBlacklist,
    getLocationText,
    getSourceLabel,
    getSourceVariant,
    handleLimitChange,
    handlePageChange,
    handleSearch,
    isAdding,
    isAllSelected,
    isDeleting,
    limit,
    loading,
    parsedAddIps,
    parsedLimit,
    records,
    searchQuery,
    selectedIps,
    showTableSkeleton,
    toggleSelect,
    totalRecords,
  };
}

export type GeneralBlacklistPageController = ReturnType<
  typeof useGeneralBlacklistPage
>;
