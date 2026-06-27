<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, Loader2, Trash2 } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import DetailFieldsGrid from "@admin-shared/components/common/DetailFieldsGrid.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import { toast } from "@admin-shared/utils/toast";
import { EventCenterAPI } from "../../lib/api";
import type {
  SystemEventLevel,
  SystemEventRecord,
  SystemEventSource,
  SystemEventType,
} from "../../types";
import {
  SYSTEM_EVENT_LEVEL_FILTER_OPTIONS as LEVEL_OPTIONS,
  SYSTEM_EVENT_SOURCE_FILTER_OPTIONS as SOURCE_OPTIONS,
  SYSTEM_EVENT_TYPE_FILTER_OPTIONS as TYPE_OPTIONS,
} from "./constants";
import { useSystemEventDisplay } from "./useSystemEventDisplay";

const props = withDefaults(
  defineProps<{
    active?: boolean;
  }>(),
  {
    active: true,
  },
);

const { t } = useI18n();

const formatOptionLabel = (option: { labelKey: string }) => t(option.labelKey);

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
  fetchPage: async ({ page, limit, query }) => {
    const result = await EventCenterAPI.getEvents({
      page,
      limit,
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
        if (currentPage.value !== 1) {
          currentPage.value = 1;
        }
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

const handleFilterChange = () => {
  currentPage.value = 1;
  fetchEvents();
};

watch([selectedType, selectedLevel, selectedSource], handleFilterChange);

const {
  describeEvent,
  detailCopyText,
  detailItems,
  eventTypeTextClass,
  formatIpDisplay,
  formatSystemEventLevelLabel,
  formatSystemEventSourceLabel,
  formatSystemEventTypeLabel,
  levelBadgeClass,
  resolveEventOrigins,
} = useSystemEventDisplay({
  activeEvent,
  translate: (key, params) => (params ? t(key, params) : t(key)),
});
onMounted(() => {
  fetchEvents();
});
</script>

<template>
  <div class="flex h-full flex-col gap-4">
    <div class="flex flex-wrap items-center gap-2">
      <SearchInput
        v-model="searchQuery"
        :placeholder="t('admin.eventCenter.events.searchPlaceholder')"
        class="w-full max-w-xs"
        @search="handleSearch"
      />

      <Select v-model="selectedType">
        <SelectTrigger class="w-[160px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.typePlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in TYPE_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Select v-model="selectedLevel">
        <SelectTrigger class="w-[140px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.levelPlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in LEVEL_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Select v-model="selectedSource">
        <SelectTrigger class="w-[110px]">
          <SelectValue
            :placeholder="t('admin.eventCenter.events.sourcePlaceholder')"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in SOURCE_OPTIONS"
            :key="option.value"
            :value="option.value"
          >
            {{ formatOptionLabel(option) }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div class="ml-auto flex items-center gap-2">
        <RefreshButton
          :loading="loading"
          :disabled="loading || isClearing"
          @click="fetchEvents"
        />

        <ConfirmDangerPopover
          :title="t('admin.eventCenter.events.clearTitle')"
          :description="clearEventsDescription"
          :confirm-text="t('admin.eventCenter.events.confirmClear')"
          :loading="isClearing"
          :disabled="loading || isClearing || totalEvents === 0"
          content-class="w-80 text-left"
          :on-confirm="clearAllEvents"
        >
          <template #trigger>
            <Button
              variant="outline"
              class="border-destructive/20 text-destructive hover:bg-destructive/5 hover:text-destructive"
              :disabled="loading || isClearing || totalEvents === 0"
            >
              <Trash2 class="mr-2 h-4 w-4" />
              {{ t("admin.eventCenter.events.clearButton") }}
            </Button>
          </template>
        </ConfirmDangerPopover>

        <ConfirmDangerPopover
          v-if="hasSelectedEvents"
          :title="
            t('admin.eventCenter.events.deleteSelectedTitle', {
              count: selectedKeys.size,
            })
          "
          :description="t('admin.eventCenter.events.deleteDescription')"
          :loading="isDeleting"
          :disabled="isDeleting || isClearing"
          :on-confirm="() => deleteEvents(Array.from(selectedKeys))"
        >
          <template #trigger>
            <Button variant="destructive" :disabled="isDeleting || isClearing">
              <Trash2 class="mr-2 h-4 w-4" />
              {{
                t("admin.eventCenter.events.deleteSelectedButton", {
                  count: selectedKeys.size,
                })
              }}
            </Button>
          </template>
        </ConfirmDangerPopover>
      </div>
    </div>

    <div
      class="flex flex-1 flex-col overflow-hidden rounded-md border bg-background"
    >
      <div class="flex-1 overflow-auto">
        <Table
          v-if="!(loading && events.length === 0)"
          class="table-fixed min-w-[980px]"
        >
          <TableHeader class="sticky top-0 z-10 bg-background shadow-sm">
            <TableRow>
              <TableHead class="w-[42px] pl-3 pr-1">
                <Checkbox v-model="isAllSelected" />
              </TableHead>
              <TableHead class="w-[300px]">
                {{ t("admin.eventCenter.events.tableEvent") }}
              </TableHead>
              <TableHead class="w-[220px]">
                {{ t("admin.eventCenter.events.origin") }}
              </TableHead>
              <TableHead class="w-[100px]">
                {{ t("admin.eventCenter.events.level") }}
              </TableHead>
              <TableHead class="w-[96px]">
                {{ t("admin.eventCenter.events.system") }}
              </TableHead>
              <TableHead class="w-[110px] pr-6 text-right">
                {{ t("admin.eventCenter.events.actions") }}
              </TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            <TableRow v-if="loading">
              <TableCell colspan="6" class="py-10 text-center">
                <Loader2
                  class="mx-auto h-6 w-6 animate-spin text-muted-foreground"
                />
              </TableCell>
            </TableRow>
            <TableRow v-else-if="events.length === 0">
              <TableCell
                colspan="6"
                class="py-10 text-center text-muted-foreground"
              >
                {{ t("admin.eventCenter.events.empty") }}
              </TableCell>
            </TableRow>
            <TableRow v-for="event in events" :key="event.id">
              <TableCell class="w-[42px] pl-3 pr-1 align-top">
                <Checkbox
                  :model-value="selectedKeys.has(event.id)"
                  @update:model-value="toggleSelect(event.id)"
                />
              </TableCell>
              <TableCell class="w-[340px] max-w-[340px] align-top">
                <div class="space-y-1.5">
                  <div class="flex items-start gap-2">
                    <div
                      class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium leading-5 text-muted-foreground"
                    >
                      <HumanFriendlyTime :value="event.happened_at" />
                    </div>
                    <div
                      class="min-w-0 text-sm font-semibold leading-6"
                      :class="eventTypeTextClass(event)"
                    >
                      {{ formatSystemEventTypeLabel(event.type) }}
                    </div>
                  </div>
                </div>
                <div
                  class="mt-1 max-w-[300px] line-clamp-3 whitespace-normal break-words text-sm leading-6 text-muted-foreground"
                >
                  {{ describeEvent(event) }}
                </div>
              </TableCell>
              <TableCell class="align-middle">
                <div
                  v-if="resolveEventOrigins(event).length === 0"
                  class="text-sm text-muted-foreground"
                >
                  -
                </div>
                <div v-else class="space-y-1">
                  <div
                    v-for="origin in resolveEventOrigins(event)"
                    :key="origin.key"
                    class="space-y-0.5 leading-5"
                  >
                    <div
                      class="font-mono text-xs text-foreground"
                      :title="origin.ip"
                    >
                      {{ formatIpDisplay(origin.ip) }}
                    </div>
                    <div
                      v-if="origin.location"
                      class="line-clamp-2 whitespace-normal text-xs leading-5 text-muted-foreground"
                    >
                      {{ origin.location }}
                    </div>
                  </div>
                </div>
              </TableCell>
              <TableCell>
                <Badge
                  variant="outline"
                  class="border px-2 py-0.5"
                  :class="levelBadgeClass(event.level)"
                >
                  {{ formatSystemEventLevelLabel(event.level) }}
                </Badge>
              </TableCell>
              <TableCell class="truncate align-middle">
                {{ formatSystemEventSourceLabel(event.source) }}
              </TableCell>
              <TableCell class="space-x-2 pr-6 text-right">
                <Button variant="ghost" size="icon" @click="viewDetails(event)">
                  <Eye class="h-4 w-4" />
                </Button>
                <ConfirmDangerPopover
                  :title="t('admin.eventCenter.events.deleteSingleTitle')"
                  :description="t('admin.eventCenter.events.deleteDescription')"
                  :loading="isDeleting"
                  :disabled="isDeleting || isClearing"
                  :on-confirm="() => deleteEvents([event.id])"
                >
                  <template #trigger>
                    <Button
                      variant="ghost"
                      size="icon"
                      class="text-destructive"
                      :disabled="isDeleting || isClearing"
                    >
                      <Trash2 class="h-4 w-4" />
                    </Button>
                  </template>
                </ConfirmDangerPopover>
              </TableCell>
            </TableRow>
          </TableBody>
        </Table>

        <TableSkeletonBlock
          v-else-if="showTableSkeleton"
          :header-widths="['w-16', 'w-52', 'w-24', 'w-12', 'w-16', 'w-10']"
          :row-widths="['w-16', 'w-56', 'w-28', 'w-12', 'w-20', 'w-10']"
        />

        <div v-else class="h-[420px]" aria-hidden="true"></div>
      </div>

      <PagedTableFooter
        :total="totalEvents"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        :total-text="t('admin.eventCenter.events.totalText')"
        :floating="props.active"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsOpen"
      :title="t('admin.eventCenter.events.detailTitle')"
      :description="t('admin.eventCenter.events.detailDescription')"
      max-width-class="sm:max-w-[760px]"
      close-variant="default"
      :copy-text="detailCopyText"
    >
      <div v-if="activeEvent" class="space-y-6">
        <DetailFieldsGrid :items="detailItems" />

        <div v-if="activeEvent.tags?.length" class="space-y-2">
          <div class="text-sm font-medium text-foreground">
            {{ t("admin.eventCenter.events.tags") }}
          </div>
          <div class="flex flex-wrap gap-2">
            <Badge
              v-for="tag in activeEvent.tags"
              :key="tag"
              variant="secondary"
              class="rounded-full px-2 py-0.5"
            >
              {{ tag }}
            </Badge>
          </div>
        </div>
      </div>
    </DetailDialog>
  </div>
</template>
