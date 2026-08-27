<script setup lang="ts">
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
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import {
  SYSTEM_EVENT_LEVEL_FILTER_OPTIONS as LEVEL_OPTIONS,
  SYSTEM_EVENT_SOURCE_FILTER_OPTIONS as SOURCE_OPTIONS,
  SYSTEM_EVENT_TYPE_FILTER_OPTIONS as TYPE_OPTIONS,
} from "./constants";
import { useSystemEventDisplay } from "./useSystemEventDisplay";
import { useSystemEvents } from "./useSystemEvents";
import TraceIdLink from "@/components/TraceIdLink.vue";

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
const {
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
} = useSystemEvents();
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
        <SelectTrigger
          :aria-label="t('admin.eventCenter.events.typePlaceholder')"
          class="w-[160px]"
        >
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
        <SelectTrigger
          :aria-label="t('admin.eventCenter.events.levelPlaceholder')"
          class="w-[140px]"
        >
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
        <SelectTrigger
          :aria-label="t('admin.eventCenter.events.sourcePlaceholder')"
          class="w-[110px]"
        >
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
                <Checkbox
                  v-model="isAllSelected"
                  :aria-label="t('common.selectAll')"
                />
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
                  :aria-label="t('common.selectItem', { item: event.id })"
                  @update:model-value="toggleSelect(event.id)"
                />
              </TableCell>
              <TableCell class="w-[340px] max-w-[340px] align-top">
                <div class="space-y-1.5">
                  <div class="flex items-center gap-2">
                    <div
                      class="inline-flex h-5 shrink-0 items-center rounded-full bg-muted px-2 text-[11px] font-medium leading-none text-muted-foreground"
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
                <Button
                  variant="ghost"
                  size="icon"
                  :aria-label="t('common.viewDetails')"
                  @click="viewDetails(event)"
                >
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
                      :aria-label="t('common.confirmDelete')"
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
        <TraceIdLink :trace-id="activeEvent.trace_id" />
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
