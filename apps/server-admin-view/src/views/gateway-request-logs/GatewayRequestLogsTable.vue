<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import GatewayRequestLogDesktopRow from "./GatewayRequestLogDesktopRow.vue";
import GatewayRequestLogMobileRow from "./GatewayRequestLogMobileRow.vue";
import type { SelectableGatewayLogEntry } from "./useGatewayLogIpSelection";
import { useSyncedHorizontalScroll } from "./useSyncedHorizontalScroll";

const props = defineProps<{
  blockIpsFromLogs: (ips: string[]) => Promise<void> | void;
  entries: SelectableGatewayLogEntry[];
  entriesCount: number;
  getConnectionSourceText: (entry: SelectableGatewayLogEntry) => string;
  getEntryIpLocationText: (entry: SelectableGatewayLogEntry) => string;
  goToWafTrace: (traceId?: string) => void;
  hasSelectableDisplayedRows: boolean;
  isAllDisplayedRowsSelected: boolean;
  isGeneralBlacklisted: (ip: string) => boolean;
  isMutatingBlacklistIps: boolean;
  loading: boolean;
  releaseIpsFromLogs: (ips: string[]) => Promise<void> | void;
  selectedLogEntryKeys: Set<string>;
  showTableSkeleton: boolean;
  toggleLogEntrySelection: (key?: string) => void;
  viewDetails: (entry: SelectableGatewayLogEntry) => void;
}>();

const emit = defineEmits<{
  "update:isAllDisplayedRowsSelected": [value: boolean];
}>();
const { t } = useI18n();
const {
  bindResizeObserver,
  canScrollLeft,
  canScrollRight,
  disposeResizeObserver,
  hasHorizontalOverflow,
  setTableScrollRef,
  setTopScrollbarRef,
  syncHorizontalScroll,
  tableContentWidth,
  tableViewportWidth,
} = useSyncedHorizontalScroll();

watch(
  [() => props.entries, () => props.loading],
  async () => {
    await nextTick();
    bindResizeObserver();
  },
  { flush: "post" },
);
onMounted(async () => {
  await nextTick();
  bindResizeObserver();
});
onUnmounted(disposeResizeObserver);
</script>

<template>
  <div
    v-if="hasHorizontalOverflow"
    class="hidden border-b px-4 py-2 md:block"
  >
    <div
      :ref="setTopScrollbarRef"
      class="overflow-x-auto overscroll-x-contain rounded-full bg-muted/35 p-1"
      @scroll="syncHorizontalScroll('top')"
    >
      <div
        class="h-1.5 rounded-full bg-foreground/20"
        :style="{
          width: `${Math.max(tableContentWidth, tableViewportWidth)}px`,
        }"
      ></div>
    </div>
  </div>

  <div class="relative flex-1 overflow-hidden">
    <div
      v-if="canScrollLeft"
      class="pointer-events-none absolute inset-y-0 left-0 z-10 w-6 bg-gradient-to-r from-background to-transparent"
    ></div>
    <div
      v-if="canScrollRight"
      class="pointer-events-none absolute inset-y-0 right-0 z-10 w-6 bg-gradient-to-l from-background to-transparent"
    ></div>

    <div
      :ref="setTableScrollRef"
      class="h-full overflow-auto overscroll-x-contain"
      @scroll="syncHorizontalScroll('table')"
    >
      <div class="divide-y md:hidden">
        <div
          v-if="loading"
          class="flex min-h-48 items-center justify-center px-4 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.gatewayRequestLogs.loading") }}
        </div>
        <div
          v-else-if="entriesCount === 0"
          class="flex min-h-48 items-center justify-center px-4 text-center text-sm text-muted-foreground"
        >
          {{ t("admin.gatewayRequestLogs.empty") }}
        </div>
        <template v-else>
          <div class="flex items-center justify-between gap-3 px-3 py-2.5">
            <label class="flex min-w-0 items-center gap-2 text-xs">
              <Checkbox
                :model-value="isAllDisplayedRowsSelected"
                :aria-label="t('common.selectAll')"
                :disabled="!hasSelectableDisplayedRows"
                @update:model-value="
                  (value) =>
                    emit('update:isAllDisplayedRowsSelected', Boolean(value))
                "
              />
              <span class="truncate">{{ t("common.selectAll") }}</span>
            </label>
            <span class="shrink-0 text-[11px] text-muted-foreground">
              {{ t("admin.gatewayRequestLogs.rowsCount", { count: entriesCount }) }}
            </span>
          </div>

          <GatewayRequestLogMobileRow
            v-for="entry in entries"
            :key="entry.selectionKey"
            :entry="entry"
            :is-selected="selectedLogEntryKeys.has(entry.selectionKey)"
            :block-ips-from-logs="blockIpsFromLogs"
            :get-connection-source-text="getConnectionSourceText"
            :get-entry-ip-location-text="getEntryIpLocationText"
            :go-to-waf-trace="goToWafTrace"
            :is-general-blacklisted="isGeneralBlacklisted"
            :is-mutating-blacklist-ips="isMutatingBlacklistIps"
            :release-ips-from-logs="releaseIpsFromLogs"
            :toggle-selection="toggleLogEntrySelection"
            :view-details="viewDetails"
          />
        </template>
      </div>

      <Table
        v-if="!(loading && entriesCount === 0)"
        class="hidden min-w-[1060px] md:table"
      >
        <TableHeader class="sticky top-0 z-10 bg-background/95 backdrop-blur">
          <TableRow>
            <TableHead
              class="h-10 w-[48px] min-w-[48px] text-[11px] font-medium text-muted-foreground"
            >
              <Checkbox
                :model-value="isAllDisplayedRowsSelected"
                :aria-label="t('common.selectAll')"
                :disabled="!hasSelectableDisplayedRows"
                @update:model-value="
                  (value) =>
                    emit('update:isAllDisplayedRowsSelected', Boolean(value))
                "
              />
            </TableHead>
            <TableHead
              class="h-10 w-[320px] min-w-[320px] max-w-[320px] text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.request") }}
            </TableHead>
            <TableHead
              class="h-10 w-[72px] min-w-[72px] text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.status") }}
            </TableHead>
            <TableHead
              class="h-10 w-[150px] min-w-[150px] max-w-[150px] text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.login") }}
            </TableHead>
            <TableHead class="h-10 text-[11px] font-medium text-muted-foreground">
              {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
            </TableHead>
            <TableHead
              class="h-10 w-[220px] min-w-[160px] max-w-[220px] text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.route") }}
            </TableHead>
            <TableHead class="h-10 text-[11px] font-medium text-muted-foreground">
              {{ t("admin.gatewayRequestLogs.columns.duration") }}
            </TableHead>
            <TableHead
              class="sticky right-0 z-20 h-10 bg-background/95 pr-4 text-right text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.actions") }}
            </TableHead>
          </TableRow>
        </TableHeader>
        <TableBody>
          <TableRow v-if="loading">
            <TableCell colspan="8" class="py-10 text-center text-muted-foreground">
              {{ t("admin.gatewayRequestLogs.loading") }}
            </TableCell>
          </TableRow>
          <TableRow v-else-if="entriesCount === 0">
            <TableCell colspan="8" class="py-10 text-center text-muted-foreground">
              {{ t("admin.gatewayRequestLogs.empty") }}
            </TableCell>
          </TableRow>
          <GatewayRequestLogDesktopRow
            v-for="entry in entries"
            v-else
            :key="entry.selectionKey"
            :entry="entry"
            :is-selected="selectedLogEntryKeys.has(entry.selectionKey)"
            :block-ips-from-logs="blockIpsFromLogs"
            :get-connection-source-text="getConnectionSourceText"
            :get-entry-ip-location-text="getEntryIpLocationText"
            :go-to-waf-trace="goToWafTrace"
            :is-general-blacklisted="isGeneralBlacklisted"
            :is-mutating-blacklist-ips="isMutatingBlacklistIps"
            :release-ips-from-logs="releaseIpsFromLogs"
            :toggle-selection="toggleLogEntrySelection"
            :view-details="viewDetails"
          />
        </TableBody>
      </Table>
      <div v-else-if="showTableSkeleton" class="hidden md:block">
        <TableSkeletonBlock
          :header-widths="['w-4', 'w-56', 'w-16', 'w-16', 'w-20', 'w-20', 'w-14', 'w-10']"
          :row-widths="['w-4', 'w-64', 'w-12', 'w-20', 'w-24', 'w-24', 'w-14', 'w-10']"
        />
      </div>
      <div v-else class="hidden h-[380px] md:block" aria-hidden="true"></div>
    </div>
  </div>
</template>
