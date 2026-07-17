import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "@/lib/api";
import type {
  SystemEventLevel,
  SystemEventRecord,
  SystemEventSource,
  SystemEventType,
} from "@/types";

export const useSystemEvents = () => {
  const { t } = useI18n();
  const selectedType = ref<SystemEventType | "all">("all");
  const selectedLevel = ref<SystemEventLevel | "all">("all");
  const selectedSource = ref<SystemEventSource | "all">("all");
  const isDetailsOpen = ref(false);
  const activeEvent = ref<SystemEventRecord | null>(null);

  const { isPending: isDeleting, run: runDelete } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.eventCenter.events.deleteFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.eventCenter.events.deleteEventFailed"),
        ),
      });
    },
  });
  const { isPending: isClearing, run: runClear } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.eventCenter.events.clearFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.eventCenter.events.clearEventFailed"),
        ),
      });
    },
  });

  const {
    items: events,
    total: totalEvents,
    loading,
    searchQuery,
    currentPage,
    limit,
    parsedLimit,
    selectedKeys,
    isAllSelected,
    fetchList: fetchEvents,
    handleSearch,
    handlePageChange,
    handleLimitChange,
    toggleSelect,
    clearSelection,
  } = usePagedSelectionList<SystemEventRecord, string>({
    fetchPage: async ({ page, limit: pageLimit, query }) => {
      const result = await EventCenterAPI.getEvents({
        page,
        limit: pageLimit,
        search: query,
        type: selectedType.value,
        level: selectedLevel.value,
        source: selectedSource.value,
      });
      if (!(result.success || result.data)) {
        throw new Error(
          result.message || t("admin.eventCenter.events.loadFailed"),
        );
      }
      return {
        items: result.data.events || [],
        total: result.data.total || 0,
      };
    },
    getKey: (event) => event.id,
    onError: (error) => {
      toast.error(t("admin.eventCenter.events.loadFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.eventCenter.events.eventListLoadFailed"),
        ),
      });
    },
  });

  const showTableSkeleton = useDelayedLoading(
    () => loading.value && events.value.length === 0,
  );
  const hasSelectedEvents = computed(() => selectedKeys.value.size > 0);
  const clearEventsDescription = computed(() =>
    t("admin.eventCenter.events.clearDescription", {
      count: totalEvents.value,
    }),
  );

  const viewDetails = (event: SystemEventRecord) => {
    activeEvent.value = event;
    isDetailsOpen.value = true;
  };
  const deleteEvents = async (ids: string[]) => {
    await runDelete(() => EventCenterAPI.deleteEvents(ids), {
      onSuccess: async (result) => {
        if (result.success || result.message === "success") {
          toast.success(t("admin.eventCenter.events.deleteSuccess"));
          clearSelection();
          await fetchEvents();
          return;
        }
        toast.error(t("admin.eventCenter.events.deleteFailed"), {
          description:
            result.message || t("admin.eventCenter.events.deleteEventFailed"),
        });
      },
    });
  };
  const clearAllEvents = async () => {
    if (totalEvents.value === 0) return;
    await runClear(() => EventCenterAPI.clearEvents(), {
      onSuccess: async (result) => {
        if (result.success || result.message === "success") {
          const deletedCount = result.data?.deleted_count ?? 0;
          toast.success(
            deletedCount > 0
              ? t("admin.eventCenter.events.clearSuccess", {
                  count: deletedCount,
                })
              : t("admin.eventCenter.events.clearEmpty"),
          );
          clearSelection();
          activeEvent.value = null;
          isDetailsOpen.value = false;
          if (currentPage.value !== 1) currentPage.value = 1;
          await fetchEvents();
          return;
        }
        toast.error(t("admin.eventCenter.events.clearFailed"), {
          description:
            result.message || t("admin.eventCenter.events.clearEventFailed"),
        });
      },
    });
  };

  watch([selectedType, selectedLevel, selectedSource], () => {
    currentPage.value = 1;
    void fetchEvents();
  });
  onMounted(() => {
    void fetchEvents();
  });

  return {
    activeEvent,
    clearAllEvents,
    clearEventsDescription,
    currentPage,
    deleteEvents,
    events,
    fetchEvents,
    handleLimitChange,
    handlePageChange,
    handleSearch,
    hasSelectedEvents,
    isAllSelected,
    isClearing,
    isDeleting,
    isDetailsOpen,
    limit,
    loading,
    parsedLimit,
    searchQuery,
    selectedKeys,
    selectedLevel,
    selectedSource,
    selectedType,
    showTableSkeleton,
    toggleSelect,
    totalEvents,
    viewDetails,
  };
};
