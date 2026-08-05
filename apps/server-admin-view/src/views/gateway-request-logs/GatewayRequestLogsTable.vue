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
  formatAuthCredential as resolveAuthCredentialLabel,
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
const authCredentialLabel = (entry: SelectableGatewayLogEntry) =>
  resolveAuthCredentialLabel(entry, t);

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
              {{
                t("admin.gatewayRequestLogs.rowsCount", {
                  count: entriesCount,
                })
              }}
            </span>
          </div>

          <article
            v-for="entry in entries"
            :key="entry.selectionKey"
            class="space-y-3 px-3 py-3"
          >
            <div class="flex items-start gap-2.5">
              <Checkbox
                class="mt-0.5"
                :model-value="selectedLogEntryKeys.has(entry.selectionKey)"
                :aria-label="
                  t('common.selectItem', {
                    item: entry.actionIp || entry.selectionKey,
                  })
                "
                :disabled="!entry.actionIp"
                @update:model-value="
                  toggleLogEntrySelection(entry.selectionKey)
                "
              />
              <div class="min-w-0 flex-1 space-y-1.5">
                <div class="flex min-w-0 items-center gap-2">
                  <span
                    class="inline-flex h-5 shrink-0 items-center rounded-full bg-muted px-2 text-[10px] font-medium leading-none text-muted-foreground"
                  >
                    <HumanFriendlyTime :value="entry.time" :locale="locale" />
                  </span>
                  <span
                    class="shrink-0 font-mono text-[10px] tracking-[0.1em] text-muted-foreground"
                  >
                    {{ entry.method || "-" }}
                  </span>
                  <span
                    class="ml-auto inline-flex shrink-0 items-center gap-1.5 font-mono text-xs"
                    :class="getStatusTextClass(entry.status)"
                  >
                    <span
                      class="h-1.5 w-1.5 rounded-full"
                      :class="getStatusDotClass(entry.status)"
                    ></span>
                    {{ entry.status }}
                  </span>
                </div>
                <p class="truncate text-sm font-medium" :title="entry.host">
                  {{ entry.host || "-" }}
                </p>
                <p
                  class="break-all font-mono text-[11px] leading-4 text-muted-foreground"
                >
                  {{ entry.request_uri || entry.path || "-" }}
                </p>
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
                  <span class="truncate font-mono">{{
                    wafBadgeMeta(entry)
                  }}</span>
                </button>
              </div>
            </div>

            <dl class="grid grid-cols-2 gap-x-3 gap-y-2 text-xs">
              <div class="min-w-0">
                <dt class="text-[10px] text-muted-foreground">
                  {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
                </dt>
                <dd class="truncate font-mono" :title="getEntryClientIp(entry)">
                  {{ getEntryClientIp(entry) || "-" }}
                </dd>
                <dd
                  v-if="getEntryIpLocationText(entry)"
                  class="truncate text-[10px] text-muted-foreground"
                  :title="getEntryIpLocationText(entry)"
                >
                  {{ getEntryIpLocationText(entry) }}
                </dd>
              </div>
              <div class="min-w-0">
                <dt class="text-[10px] text-muted-foreground">
                  {{ t("admin.gatewayRequestLogs.columns.login") }}
                </dt>
                <dd class="truncate">
                  {{
                    entry.logged_in
                      ? t("admin.gatewayRequestLogs.loggedIn")
                      : t("admin.gatewayRequestLogs.notLoggedIn")
                  }}
                </dd>
                <dd
                  class="truncate text-[10px] text-muted-foreground"
                  :title="authDecisionLabel(entry.auth_decision)"
                >
                  {{ authDecisionLabel(entry.auth_decision) }}
                </dd>
              </div>
              <div class="min-w-0">
                <dt class="text-[10px] text-muted-foreground">
                  {{ t("admin.gatewayRequestLogs.columns.route") }}
                </dt>
                <dd class="truncate" :title="routeTypeLabel(entry.route_type)">
                  {{ routeTypeLabel(entry.route_type) }}
                </dd>
                <dd
                  class="truncate text-[10px] text-muted-foreground"
                  :title="entry.route_key || '-'"
                >
                  {{ entry.route_key || "-" }}
                </dd>
              </div>
              <div class="min-w-0">
                <dt class="text-[10px] text-muted-foreground">
                  {{ t("admin.gatewayRequestLogs.columns.duration") }}
                </dt>
                <dd class="font-mono">{{ formatDuration(entry.duration_ms) }}</dd>
              </div>
            </dl>

            <div class="flex items-center justify-end gap-1 border-t pt-2">
              <ConfirmDangerPopover
                :title="
                  isGeneralBlacklisted(entry.actionIp)
                    ? t('admin.gatewayRequestLogs.unblacklistOneTitle')
                    : t('admin.gatewayRequestLogs.blacklistOneTitle')
                "
                :description="
                  isGeneralBlacklisted(entry.actionIp)
                    ? t('admin.gatewayRequestLogs.unblacklistOneDescription', {
                        ip: entry.actionIp || '-',
                      })
                    : t('admin.gatewayRequestLogs.blacklistOneDescription', {
                        ip: entry.actionIp || '-',
                      })
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
                    size="sm"
                    class="h-8 px-2.5 text-xs"
                    :class="
                      isGeneralBlacklisted(entry.actionIp)
                        ? 'text-foreground hover:text-foreground'
                        : 'text-destructive hover:text-destructive'
                    "
                    :disabled="!entry.actionIp || isMutatingBlacklistIps"
                  >
                    <Unlock
                      v-if="isGeneralBlacklisted(entry.actionIp)"
                      class="mr-1.5 h-3.5 w-3.5"
                    />
                    <Ban v-else class="mr-1.5 h-3.5 w-3.5" />
                    {{
                      isGeneralBlacklisted(entry.actionIp)
                        ? t("admin.gatewayRequestLogs.unblacklistOne")
                        : t("admin.gatewayRequestLogs.blacklistOne")
                    }}
                  </Button>
                </template>
              </ConfirmDangerPopover>
              <Button
                variant="ghost"
                size="sm"
                class="h-8 px-2.5 text-xs text-muted-foreground hover:text-foreground"
                @click="viewDetails(entry)"
              >
                <Eye class="mr-1.5 h-3.5 w-3.5" />
                {{ t("common.viewDetails") }}
              </Button>
            </div>
          </article>
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
            <TableHead
              class="h-10 text-[11px] font-medium text-muted-foreground"
            >
              {{ t("admin.gatewayRequestLogs.columns.clientIp") }}
            </TableHead>
            <TableHead
              class="h-10 w-[220px] min-w-[160px] max-w-[220px] text-[11px] font-medium text-muted-foreground"
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
                :aria-label="
                  t('common.selectItem', {
                    item: entry.actionIp || entry.selectionKey,
                  })
                "
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
                <div class="flex items-center gap-2">
                  <div
                    class="inline-flex h-5 shrink-0 items-center rounded-full bg-muted px-2 text-[11px] font-medium leading-none text-muted-foreground"
                  >
                    <HumanFriendlyTime :value="entry.time" :locale="locale" />
                  </div>
                  <div class="min-w-0 flex-1">
                    <div
                      class="flex items-center gap-2 text-sm text-foreground"
                    >
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
                  <span class="truncate font-mono">{{
                    wafBadgeMeta(entry)
                  }}</span>
                </button>
              </div>
            </TableCell>
            <TableCell class="w-[72px] min-w-[72px] py-2.5">
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
            <TableCell class="w-[150px] min-w-[150px] max-w-[150px] py-2.5">
              <div class="truncate text-sm text-foreground">
                {{
                  entry.logged_in
                    ? t("admin.gatewayRequestLogs.loggedIn")
                    : t("admin.gatewayRequestLogs.notLoggedIn")
                }}
              </div>
              <div class="truncate text-[11px] text-muted-foreground">
                {{ authDecisionLabel(entry.auth_decision) }}
              </div>
              <div
                v-if="authCredentialLabel(entry)"
                class="max-w-full truncate text-[11px] text-muted-foreground/75"
                :title="authCredentialLabel(entry)"
              >
                {{ authCredentialLabel(entry) }}
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
            <TableCell
              class="w-[220px] min-w-[160px] max-w-[220px] overflow-hidden py-2.5"
            >
              <div class="w-full max-w-[204px] overflow-hidden">
                <div
                  class="truncate text-sm text-foreground"
                  :title="routeTypeLabel(entry.route_type)"
                >
                  {{ routeTypeLabel(entry.route_type) }}
                </div>
                <div
                  class="truncate text-[11px] text-muted-foreground"
                  :title="entry.route_key || '-'"
                >
                  {{ entry.route_key || "-" }}
                </div>
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
                  class="opacity-60 transition-opacity group-hover:opacity-100 group-focus-within:opacity-100"
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
      <div v-else-if="showTableSkeleton" class="hidden md:block">
        <TableSkeletonBlock
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
      </div>
      <div v-else class="hidden h-[380px] md:block" aria-hidden="true"></div>
    </div>
  </div>
</template>
