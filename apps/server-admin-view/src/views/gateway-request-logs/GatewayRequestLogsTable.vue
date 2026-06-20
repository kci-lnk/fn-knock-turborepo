<script setup lang="ts">
import { nextTick, onMounted, onUnmounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import {
  Ban,
  Eye,
  ShieldAlert,
  ShieldCheck,
  ShieldX,
  Unlock,
} from "lucide-vue-next";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import {
  authDecisionLabel as resolveAuthDecisionLabel,
  formatDuration,
  getEntryClientIp,
  getForwardedHeaderLines,
  getStatusDotClass,
  getStatusTextClass,
  getWAFAction,
  getWAFBadgeClass,
  hasWAFSignal,
  isWAFBlocked,
  routeTypeLabel as resolveRouteTypeLabel,
  wafBadgeLabel as resolveWAFBadgeLabel,
  wafBadgeMeta as resolveWAFBadgeMeta,
  wafBadgeTitle as resolveWAFBadgeTitle,
} from "./model";
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

const { t, locale } = useI18n();
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

const wafBadgeLabel = (entry: SelectableGatewayLogEntry) =>
  resolveWAFBadgeLabel(entry, t);
const wafBadgeMeta = (entry: SelectableGatewayLogEntry) =>
  resolveWAFBadgeMeta(entry, t);
const wafBadgeTitle = (entry: SelectableGatewayLogEntry) =>
  resolveWAFBadgeTitle(entry, t);
const routeTypeLabel = (value?: string) => resolveRouteTypeLabel(value, t);
const authDecisionLabel = (value?: string) =>
  resolveAuthDecisionLabel(value, t);

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
  <div v-if="hasHorizontalOverflow" class="border-b px-4 py-2">
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
      <Table v-if="!(loading && entriesCount === 0)" class="min-w-[1040px]">
        <TableHeader class="sticky top-0 z-10 bg-background/95 backdrop-blur">
          <TableRow>
            <TableHead
              class="h-10 w-[48px] min-w-[48px] text-[11px] font-medium text-muted-foreground"
            >
              <Checkbox
                :model-value="isAllDisplayedRowsSelected"
                :disabled="!hasSelectableDisplayedRows"
                @update:model-value="
                  (value) =>
                    emit(
                      'update:isAllDisplayedRowsSelected',
                      Boolean(value),
                    )
                "
              />
            </TableHead>
            <TableHead
              class="h-10 w-[320px] min-w-[320px] max-w-[320px] text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.request") }}
            </TableHead>
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.status") }}
            </TableHead>
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.login") }}
            </TableHead>
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
            </TableHead>
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.route") }}
            </TableHead>
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
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
            <TableCell
              colspan="8"
              class="py-10 text-center text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.loading") }}
            </TableCell>
          </TableRow>
          <TableRow v-else-if="entriesCount === 0">
            <TableCell
              colspan="8"
              class="py-10 text-center text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.empty") }}
            </TableCell>
          </TableRow>
          <TableRow
            v-for="entry in entries"
            v-else
            :key="entry.selectionKey"
            class="group align-top"
          >
            <TableCell class="py-2.5">
              <Checkbox
                :model-value="selectedLogEntryKeys.has(entry.selectionKey)"
                :disabled="!entry.actionIp"
                @update:model-value="
                  toggleLogEntrySelection(entry.selectionKey)
                "
              />
            </TableCell>
            <TableCell
              class="w-[320px] min-w-[320px] max-w-[320px] whitespace-normal py-2.5"
            >
              <div class="space-y-1.5">
                <div class="flex items-start gap-2">
                  <div
                    class="shrink-0 rounded-full bg-muted px-2 py-0.5 text-[11px] font-medium leading-5 text-muted-foreground"
                  >
                    <HumanFriendlyTime :value="entry.time" :locale="locale" />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div class="flex items-center gap-2 text-sm text-foreground">
                      <span
                        class="font-mono text-[11px] tracking-[0.12em] text-muted-foreground"
                      >
                        {{ entry.method || "-" }}
                      </span>
                      <span class="min-w-0 flex-1 truncate">
                        {{ entry.host || "-" }}
                      </span>
                    </div>
                  </div>
                </div>
                <div
                  class="whitespace-normal break-all font-mono text-[11px] leading-5 text-muted-foreground"
                >
                  {{ entry.request_uri || entry.path || "-" }}
                </div>
                <div
                  v-if="entry.upstream"
                  class="whitespace-normal break-all text-[11px] text-muted-foreground/75"
                >
                  {{ entry.upstream }}
                </div>
                <button
                  v-if="hasWAFSignal(entry)"
                  type="button"
                  class="inline-flex max-w-full items-center gap-1 rounded-full border px-1.5 py-px text-[10px] font-normal leading-4 transition-colors disabled:cursor-default disabled:opacity-70"
                  :class="getWAFBadgeClass(entry)"
                  :title="wafBadgeTitle(entry)"
                  :disabled="!entry.waf_trace_id"
                  @click.stop="goToWafTrace(entry.waf_trace_id)"
                >
                  <ShieldX
                    v-if="isWAFBlocked(entry)"
                    class="h-2.5 w-2.5 shrink-0"
                  />
                  <ShieldCheck
                    v-else-if="getWAFAction(entry) === 'pass'"
                    class="h-2.5 w-2.5 shrink-0"
                  />
                  <ShieldAlert v-else class="h-2.5 w-2.5 shrink-0" />
                  <span class="shrink-0">{{ wafBadgeLabel(entry) }}</span>
                  <span class="truncate font-mono">{{ wafBadgeMeta(entry) }}</span>
                </button>
              </div>
            </TableCell>
            <TableCell class="py-2.5">
              <div
                class="flex items-center gap-2 font-mono text-sm"
                :class="getStatusTextClass(entry.status)"
              >
                <span
                  class="h-1.5 w-1.5 rounded-full"
                  :class="getStatusDotClass(entry.status)"
                ></span>
                <span>{{ entry.status }}</span>
              </div>
            </TableCell>
            <TableCell class="py-2.5">
              <div class="text-sm text-foreground">
                {{
                  entry.logged_in
                    ? t("admin.gatewayRequestLogs.loggedIn")
                    : t("admin.gatewayRequestLogs.notLoggedIn")
                }}
              </div>
              <div class="text-[11px] text-muted-foreground">
                {{ authDecisionLabel(entry.auth_decision) }}
              </div>
            </TableCell>
            <TableCell class="min-w-[140px] py-2.5">
              <div class="font-mono text-sm text-foreground">
                {{ getEntryClientIp(entry) || "-" }}
              </div>
              <div
                v-if="getConnectionSourceText(entry)"
                class="break-all text-[10px] text-muted-foreground/75"
              >
                {{ getConnectionSourceText(entry) }}
              </div>
              <div
                v-if="getEntryIpLocationText(entry)"
                class="text-[11px] text-muted-foreground"
              >
                {{ getEntryIpLocationText(entry) }}
              </div>
              <div
                v-for="headerLine in getForwardedHeaderLines(entry)"
                :key="headerLine"
                class="break-all text-[10px] text-muted-foreground/75"
              >
                {{ headerLine }}
              </div>
            </TableCell>
            <TableCell class="min-w-[110px] py-2.5">
              <div class="text-sm text-foreground">
                {{ routeTypeLabel(entry.route_type) }}
              </div>
              <div class="break-all text-[11px] text-muted-foreground">
                {{ entry.route_key || "-" }}
              </div>
            </TableCell>
            <TableCell
              class="whitespace-nowrap py-2.5 font-mono text-sm text-muted-foreground"
            >
              {{ formatDuration(entry.duration_ms) }}
            </TableCell>
            <TableCell
              class="sticky right-0 z-10 bg-background py-2.5 pr-4 text-right"
            >
              <div class="flex justify-end gap-1">
                <div
                  class="pointer-events-none opacity-0 transition-opacity group-hover:pointer-events-auto group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:opacity-100"
                >
                  <ConfirmDangerPopover
                    :title="
                      isGeneralBlacklisted(entry.actionIp)
                        ? t('admin.gatewayRequestLogs.unblacklistOneTitle')
                        : t('admin.gatewayRequestLogs.blacklistOneTitle')
                    "
                    :description="
                      isGeneralBlacklisted(entry.actionIp)
                        ? t(
                            'admin.gatewayRequestLogs.unblacklistOneDescription',
                            { ip: entry.actionIp || '-' },
                          )
                        : t(
                            'admin.gatewayRequestLogs.blacklistOneDescription',
                            { ip: entry.actionIp || '-' },
                          )
                    "
                    :loading="isMutatingBlacklistIps"
                    :disabled="!entry.actionIp || isMutatingBlacklistIps"
                    :on-confirm="
                      () =>
                        isGeneralBlacklisted(entry.actionIp)
                          ? releaseIpsFromLogs([entry.actionIp])
                          : blockIpsFromLogs([entry.actionIp])
                    "
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="icon"
                        class="h-8 w-8"
                        :class="
                          isGeneralBlacklisted(entry.actionIp)
                            ? 'text-foreground hover:text-foreground'
                            : 'text-destructive hover:text-destructive'
                        "
                        :disabled="!entry.actionIp || isMutatingBlacklistIps"
                        :aria-label="
                          isGeneralBlacklisted(entry.actionIp)
                            ? t('admin.gatewayRequestLogs.unblacklistOne')
                            : t('admin.gatewayRequestLogs.blacklistOne')
                        "
                      >
                        <Unlock
                          v-if="isGeneralBlacklisted(entry.actionIp)"
                          class="h-4 w-4"
                        />
                        <Ban v-else class="h-4 w-4" />
                      </Button>
                    </template>
                  </ConfirmDangerPopover>
                </div>
                <Button
                  variant="ghost"
                  size="icon"
                  class="h-8 w-8 text-muted-foreground hover:text-foreground"
                  :aria-label="t('common.viewDetails')"
                  @click="viewDetails(entry)"
                >
                  <Eye class="h-4 w-4" />
                </Button>
              </div>
            </TableCell>
          </TableRow>
        </TableBody>
      </Table>
      <TableSkeletonBlock
        v-else-if="showTableSkeleton"
        :header-widths="[
          'w-4',
          'w-56',
          'w-16',
          'w-16',
          'w-20',
          'w-20',
          'w-14',
          'w-10',
        ]"
        :row-widths="[
          'w-4',
          'w-64',
          'w-12',
          'w-20',
          'w-24',
          'w-24',
          'w-14',
          'w-10',
        ]"
      />
      <div v-else class="h-[380px]" aria-hidden="true"></div>
    </div>
  </div>
</template>
