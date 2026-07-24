<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Ban, Eye, Unlock } from "lucide-vue-next";
import { Badge } from "@/components/ui/badge";
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
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import type { WAFEvent } from "@/types";
import type { SelectableWafLogEntry } from "./useWafLogIpSelection";

defineProps<{
  actionLabel: (value?: string) => string;
  actionVariant: (
    value?: string,
  ) => "default" | "secondary" | "destructive" | "outline";
  blockIpsFromWafLogs: (ips: string[]) => Promise<void> | void;
  entries: SelectableWafLogEntry[];
  formatPrimaryRuleId: (entry: WAFEvent) => string;
  formatRuleLocationSummary: (entry: WAFEvent) => string;
  formatRuleSummary: (entry: WAFEvent) => string;
  getEntryDisplayIp: (entry: WAFEvent) => string;
  getEntryIpLocationText: (entry: WAFEvent) => string;
  hasSelectableDisplayedRows: boolean;
  isAllDisplayedRowsSelected: boolean;
  isGeneralBlacklisted: (ip: string) => boolean;
  isMutatingBlacklistIps: boolean;
  loading: boolean;
  modeLabel: (value?: string) => string;
  releaseIpsFromWafLogs: (ips: string[]) => Promise<void> | void;
  routeTypeLabel: (value?: string) => string;
  selectedWafEntryKeys: Set<string>;
  toggleWafEntrySelection: (key?: string) => void;
  viewDetails: (entry: WAFEvent) => void;
}>();

const emit = defineEmits<{
  "update:isAllDisplayedRowsSelected": [value: boolean];
}>();

const { locale, t } = useI18n();
</script>

<template>
  <div class="min-h-0 flex-1 overflow-auto">
    <Table class="min-w-[880px]">
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
            >{{ t("admin.wafLogs.requestColumn") }}</TableHead
          >
          <TableHead
            class="h-10 text-[11px] font-medium text-muted-foreground"
            >{{ t("admin.wafLogs.sourceColumn") }}</TableHead
          >
          <TableHead
            class="h-10 min-w-[220px] text-[11px] font-medium text-muted-foreground"
            >{{ t("admin.wafLogs.rulesColumn") }}</TableHead
          >
          <TableHead
            class="sticky right-0 z-20 h-10 bg-background/95 pr-4 text-right text-[11px] font-medium text-muted-foreground"
            >{{ t("admin.wafLogs.actionColumn") }}</TableHead
          >
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-if="loading && entries.length === 0">
          <TableCell
            colspan="5"
            class="py-10 text-center text-muted-foreground"
          >
            {{ t("admin.wafLogs.loading") }}
          </TableCell>
        </TableRow>
        <TableRow v-else-if="entries.length === 0">
          <TableCell
            colspan="5"
            class="py-10 text-center text-muted-foreground"
          >
            {{ t("admin.wafLogs.empty") }}
          </TableCell>
        </TableRow>
        <TableRow
          v-else
          v-for="entry in entries"
          :key="entry.selectionKey"
          class="group align-top"
        >
          <TableCell class="py-2.5">
            <Checkbox
              :model-value="selectedWafEntryKeys.has(entry.selectionKey)"
              :aria-label="
                t('common.selectItem', {
                  item: entry.actionIp || entry.selectionKey,
                })
              "
              :disabled="!entry.actionIp"
              @update:model-value="toggleWafEntrySelection(entry.selectionKey)"
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
                <Badge :variant="actionVariant(entry.action)" class="shrink-0">
                  {{ actionLabel(entry.action) }}
                </Badge>
                <div class="min-w-0 flex-1">
                  <div class="flex items-center gap-2 text-sm text-foreground">
                    <span
                      class="font-mono text-[11px] tracking-[0.12em] text-muted-foreground"
                    >
                      {{ entry.method || "-" }}
                    </span>
                    <span class="min-w-0 flex-1 truncate">{{
                      entry.host || "-"
                    }}</span>
                  </div>
                </div>
              </div>
              <div
                class="whitespace-normal break-all font-mono text-[11px] leading-5 text-muted-foreground"
              >
                {{ entry.request_uri || entry.path || "-" }}
              </div>
              <div class="flex flex-wrap items-center gap-x-2 gap-y-1">
                <span class="text-[11px] text-muted-foreground">
                  {{ routeTypeLabel(entry.route_type) }}
                </span>
                <span class="text-[11px] text-muted-foreground">
                  {{ modeLabel(entry.mode) }}
                </span>
                <span
                  v-if="entry.status"
                  class="font-mono text-[11px] text-muted-foreground"
                >
                  HTTP {{ entry.status }}
                </span>
                <span
                  v-if="entry.route_key"
                  class="break-all text-[11px] text-muted-foreground/75"
                >
                  {{ entry.route_key }}
                </span>
              </div>
            </div>
          </TableCell>
          <TableCell class="min-w-[150px] py-2.5">
            <div class="font-mono text-sm text-foreground">
              {{ getEntryDisplayIp(entry) }}
            </div>
            <div
              v-if="getEntryIpLocationText(entry)"
              class="text-[11px] text-muted-foreground"
            >
              {{ getEntryIpLocationText(entry) }}
            </div>
          </TableCell>
          <TableCell class="py-2.5">
            <div class="font-mono text-xs text-foreground">
              {{ formatPrimaryRuleId(entry) }}
            </div>
            <div
              v-if="formatRuleSummary(entry)"
              class="mt-1 line-clamp-2 text-[11px] leading-5 text-muted-foreground"
            >
              {{ formatRuleSummary(entry) }}
            </div>
            <div
              v-if="formatRuleLocationSummary(entry)"
              class="mt-1 line-clamp-1 break-all font-mono text-[10px] leading-4 text-muted-foreground/75"
            >
              {{ formatRuleLocationSummary(entry) }}
            </div>
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
                      ? t('admin.wafLogs.unblacklistOneTitle')
                      : t('admin.wafLogs.blacklistOneTitle')
                  "
                  :description="
                    isGeneralBlacklisted(entry.actionIp)
                      ? t('admin.wafLogs.unblacklistOneDescription', {
                          ip: entry.actionIp || '-',
                        })
                      : t('admin.wafLogs.blacklistOneDescription', {
                          ip: entry.actionIp || '-',
                        })
                  "
                  :loading="isMutatingBlacklistIps"
                  :disabled="!entry.actionIp || isMutatingBlacklistIps"
                  :on-confirm="
                    () =>
                      isGeneralBlacklisted(entry.actionIp)
                        ? releaseIpsFromWafLogs([entry.actionIp])
                        : blockIpsFromWafLogs([entry.actionIp])
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
                          ? t('admin.wafLogs.unblacklistOne')
                          : t('admin.wafLogs.blacklistOne')
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
  </div>
</template>
