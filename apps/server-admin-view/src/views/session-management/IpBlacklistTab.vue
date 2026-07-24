<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { Button } from "@/components/ui/button";
import RefreshButton from "@/components/RefreshButton.vue";
import SearchInput from "@admin-shared/components/SearchInput.vue";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { Checkbox } from "@/components/ui/checkbox";
import { Badge } from "@/components/ui/badge";
import { toast } from "@admin-shared/utils/toast";
import { Ban, Eye, Loader2, Settings, Trash2 } from "lucide-vue-next";
import {
  ScannerAPI,
  SecurityAPI,
  type ScannerBlacklistRecord,
} from "../../lib/api";
import {
  DEFAULT_THREAT_RANGES,
  useThreatOverview,
} from "@admin-shared/composables/useThreatOverview";
import { usePagedSelectionList } from "@admin-shared/composables/usePagedSelectionList";
import ThreatOverviewCard from "@admin-shared/components/common/ThreatOverviewCard.vue";
import PagedTableFooter from "@admin-shared/components/list/PagedTableFooter.vue";
import ConfirmDangerPopover from "@admin-shared/components/common/ConfirmDangerPopover.vue";
import DetailDialog from "@admin-shared/components/common/DetailDialog.vue";
import TableSkeletonBlock from "@admin-shared/components/list/TableSkeletonBlock.vue";
import BlacklistHitsTable from "@admin-shared/components/session/BlacklistHitsTable.vue";
import HumanFriendlyTime from "@admin-shared/components/common/HumanFriendlyTime.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { formatDateTimeSafe } from "@admin-shared/utils/formatDateTimeSafe";
import ConfigCollapsibleCard from "@admin-shared/components/ConfigCollapsibleCard.vue";
import TimeSeriesChart, {
  type TimeSeriesChartSeries,
} from "@/components/charts/TimeSeriesChart.vue";

const { t, locale } = useI18n();
const ranges = DEFAULT_THREAT_RANGES;
const formatOverviewRangeText = (seconds: number) => {
  if (seconds < 3600) {
    return t("admin.components.threatOverview.rangeMinutes", {
      count: Math.round(seconds / 60),
    });
  }
  if (seconds < 24 * 3600) {
    return t("admin.components.threatOverview.rangeHours", {
      count: Math.round(seconds / 3600),
    });
  }
  return t("admin.components.threatOverview.rangeDays", {
    count: Math.round(seconds / 86400),
  });
};

const {
  rangeKey,
  threatOverview,
  isThreatLoading,
  titleRangeText,
  perHour: blockedPerHour,
  formatNumber,
  formatRate,
  fetchThreatOverview,
} = useThreatOverview({
  defaultRangeKey: "1h",
  ranges,
  seriesKey: "blockedScanners",
  fetchOverview: (rangeSec) => SecurityAPI.getOverview(rangeSec),
  onError: (err: any) => {
    const msg =
      err?.response?.data?.message ||
      err?.message ||
      t("admin.sessions.ipBlacklist.loadFailed");
    toast.error(t("admin.sessions.ipBlacklist.threatLoadFailed"), {
      description: msg,
    });
  },
  formatRangeText: formatOverviewRangeText,
  numberLocale: () => locale.value,
});

const blockedTrendSeries = computed<TimeSeriesChartSeries[]>(() => [
  {
    name: t("admin.sessions.ipBlacklist.seriesName"),
    color: "#f97316",
    fill: "rgba(249, 115, 22, 0.14)",
    data: threatOverview.value?.series.blockedScanners ?? [],
  },
]);

const router = useRouter();
const { isPending: isDeleting, run: runDeleteAction } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.ipBlacklist.deleteFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.ipBlacklist.deleteFailed"),
      ),
    });
  },
});
const { isPending: isDetailLoading, run: runLoadDetail } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.sessions.ipBlacklist.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.sessions.ipBlacklist.detailLoadFailed"),
      ),
    });
    detailRecord.value = null;
  },
});

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
} = usePagedSelectionList<ScannerBlacklistRecord, string>({
  fetchPage: async ({ page, limit, query }) => {
    const data = await ScannerAPI.getBlacklist(page, limit, query);
    return {
      items: data.items || [],
      total: data.total || 0,
    };
  },
  getKey: (record) => record.ip,
  onError: (err: any) => {
    const msg =
      err?.response?.data?.message ||
      err?.message ||
      t("admin.sessions.ipBlacklist.loadFailed");
    toast.error(t("admin.sessions.ipBlacklist.loadFailed"), {
      description: msg,
    });
  },
});

const isDetailsModalOpen = ref(false);
const detailRecord = ref<ScannerBlacklistRecord | null>(null);
const showTableSkeleton = useDelayedLoading(
  () => loading.value && records.value.length === 0,
);

const deleteBlacklist = async (ips: string[]) => {
  if (ips.length === 0) return;
  await runDeleteAction(() => ScannerAPI.deleteBlacklist(ips), {
    onSuccess: async () => {
      toast.success(t("admin.sessions.ipBlacklist.deleteSuccess"));
      clearSelection();
      await fetchBlacklist();
    },
  });
};

const deleteOne = async (ip: string) => {
  await runDeleteAction(() => ScannerAPI.deleteBlacklistByIp(ip), {
    onSuccess: async () => {
      toast.success(t("admin.sessions.ipBlacklist.deleteSuccess"));
      selectedIps.value.delete(ip);
      selectedIps.value = new Set(selectedIps.value);
      await fetchBlacklist();
    },
  });
};

const viewDetails = async (record: ScannerBlacklistRecord) => {
  isDetailsModalOpen.value = true;
  await runLoadDetail(() => ScannerAPI.getBlacklistDetail(record.ip), {
    onSuccess: (detail) => {
      detailRecord.value = detail;
    },
  });
};

const formatDate = (ts?: number) => {
  return formatDateTimeSafe(ts, { locale: locale.value });
};

const formatIntervalSeconds = (value: number | null) => {
  if (value === null) return "-";
  if (!Number.isFinite(value)) return "-";
  return t("admin.sessions.ipBlacklist.seconds", {
    seconds: (value * 60).toFixed(2),
  });
};

const detailHits = computed(() => {
  if (!detailRecord.value?.hits) return [];
  const sorted = [...detailRecord.value.hits].sort(
    (a, b) => a.createdAt - b.createdAt,
  );
  return sorted.map((hit, index) => {
    const prev = sorted[index - 1];
    const intervalMinutes = prev
      ? (hit.createdAt - prev.createdAt) / 60000
      : null;
    return { ...hit, intervalMinutes };
  });
});

const detailHitRows = computed(() =>
  detailHits.value.map((hit, index) => ({
    key: `${hit.createdAt}-${index}`,
    time: formatDate(hit.createdAt),
    path: hit.path,
    interval: formatIntervalSeconds(hit.intervalMinutes),
  })),
);

onMounted(() => {
  fetchBlacklist();
  fetchThreatOverview();
});

const goToFirewallSettings = () => {
  router.push({ path: "/system", query: { tab: "scanner-firewall" } });
};
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <ConfigCollapsibleCard
      :title="t('admin.sessions.ipBlacklist.chartTitle')"
      :configured="true"
      :edit-label="t('admin.sessions.ipBlacklist.expandChart')"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{
          t("admin.sessions.ipBlacklist.chartSummary", {
            range: titleRangeText,
            count: formatNumber(threatOverview?.totals?.blockedScanners),
          })
        }}
      </template>

      <template #default>
        <ThreatOverviewCard
          v-model:range-key="rangeKey"
          :title="t('admin.sessions.ipBlacklist.overviewTitle')"
          :description="t('admin.sessions.ipBlacklist.overviewDescription')"
          :ranges="ranges"
          :is-loading="isThreatLoading"
          :title-range-text="titleRangeText"
          :primary-label="t('admin.sessions.ipBlacklist.primaryLabel')"
          :primary-value="formatNumber(threatOverview?.totals?.blockedScanners)"
          :primary-hint="t('admin.sessions.ipBlacklist.primaryHint')"
          :secondary-label="t('admin.sessions.ipBlacklist.secondaryLabel')"
          :secondary-value="formatRate(blockedPerHour)"
          :secondary-hint="t('admin.sessions.ipBlacklist.secondaryHint')"
          :icon="Ban"
        >
          <template #chart>
            <TimeSeriesChart
              :series="blockedTrendSeries"
              :value-formatter="(value) => formatNumber(value)"
              class="h-full w-full"
            />
          </template>
        </ThreatOverviewCard>
      </template>
    </ConfigCollapsibleCard>

    <div class="flex items-center gap-2">
      <SearchInput
        v-model="searchQuery"
        :placeholder="t('admin.sessions.ipBlacklist.searchPlaceholder')"
        class="w-[260px]"
        @search="handleSearch"
      />
      <div class="flex-1"></div>
      <RefreshButton
        :loading="loading"
        :disabled="loading"
        @click="fetchBlacklist"
      />
      <Button variant="outline" @click="goToFirewallSettings">
        <Settings class="h-4" />
        {{ t("admin.sessions.ipBlacklist.settings") }}
      </Button>
      <ConfirmDangerPopover
        :title="
          t('admin.sessions.ipBlacklist.deleteSelectedTitle', {
            count: selectedIps.size,
          })
        "
        :description="t('admin.sessions.ipBlacklist.deleteDescription')"
        :loading="isDeleting"
        :disabled="selectedIps.size === 0 || isDeleting"
        :on-confirm="() => deleteBlacklist(Array.from(selectedIps))"
      >
        <template #trigger>
          <Button
            variant="destructive"
            :disabled="selectedIps.size === 0 || isDeleting"
          >
            <Trash2 class="h-4" />
            {{
              t("admin.sessions.ipBlacklist.deleteSelected", {
                count: selectedIps.size,
              })
            }}
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>

    <div
      class="border rounded-md overflow-hidden bg-background flex-1 flex flex-col"
    >
      <div class="flex-1 w-full overflow-hidden">
        <div class="h-full overflow-auto">
          <Table v-if="!(loading && records.length === 0)">
            <TableHeader class="sticky top-0 bg-background z-10 shadow-sm">
              <TableRow>
                <TableHead class="w-[50px]">
                  <Checkbox
                    v-model="isAllSelected"
                    :aria-label="t('common.selectAll')"
                  />
                </TableHead>
                <TableHead>{{
                  t("admin.sessions.ipBlacklist.ipLocationHeader")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.ipBlacklist.blockedAt")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.ipBlacklist.window")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.ipBlacklist.threshold")
                }}</TableHead>
                <TableHead>{{
                  t("admin.sessions.ipBlacklist.hits")
                }}</TableHead>
                <TableHead class="text-right pr-6">{{
                  t("admin.sessions.table.actions")
                }}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="7" class="text-center py-10">
                  <Loader2
                    class="h-6 w-6 animate-spin mx-auto text-muted-foreground"
                  />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="records.length === 0">
                <TableCell
                  colspan="7"
                  class="text-center py-10 text-muted-foreground"
                >
                  {{ t("admin.sessions.ipBlacklist.empty") }}
                </TableCell>
              </TableRow>
              <TableRow v-else v-for="record in records" :key="record.ip">
                <TableCell>
                  <Checkbox
                    :model-value="selectedIps.has(record.ip)"
                    :aria-label="t('common.selectItem', { item: record.ip })"
                    @update:model-value="toggleSelect(record.ip)"
                  />
                </TableCell>
                <TableCell class="font-medium">
                  <div class="font-mono text-sm">{{ record.ip }}</div>
                  <div
                    v-if="record.ipLocation"
                    class="text-xs text-muted-foreground mt-0.5 break-all"
                  >
                    {{ record.ipLocation }}
                  </div>
                </TableCell>
                <TableCell class="whitespace-nowrap">
                  <HumanFriendlyTime
                    :value="record.blockedAt"
                    :locale="locale"
                  />
                </TableCell>
                <TableCell>{{
                  t("admin.sessions.ipBlacklist.minutes", {
                    count: record.windowMinutes,
                  })
                }}</TableCell>
                <TableCell>{{
                  t("admin.sessions.ipBlacklist.times", {
                    count: record.threshold,
                  })
                }}</TableCell>
                <TableCell>
                  <Badge variant="secondary">{{
                    record.hits?.length || 0
                  }}</Badge>
                </TableCell>
                <TableCell class="text-right space-x-2 pr-6">
                  <Button
                    variant="ghost"
                    size="icon"
                    :aria-label="t('common.viewDetails')"
                    @click="viewDetails(record)"
                  >
                    <Eye class="h-4 w-4" />
                  </Button>
                  <ConfirmDangerPopover
                    :title="t('admin.sessions.ipBlacklist.deleteOneTitle')"
                    :description="
                      t('admin.sessions.ipBlacklist.deleteDescription')
                    "
                    :loading="isDeleting"
                    :disabled="isDeleting"
                    :on-confirm="() => deleteOne(record.ip)"
                  >
                    <template #trigger>
                      <Button
                        variant="ghost"
                        size="icon"
                        :aria-label="t('common.confirmDelete')"
                        class="text-destructive"
                        :disabled="isDeleting"
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
            :header-widths="[
              'w-4',
              'w-24',
              'w-20',
              'w-10',
              'w-10',
              'w-10',
              'w-10',
            ]"
            :row-widths="[
              'w-4',
              'w-24',
              'w-20',
              'w-10',
              'w-10',
              'w-10',
              'w-16',
            ]"
          />
          <div v-else class="h-[380px]" aria-hidden="true"></div>
        </div>
      </div>

      <PagedTableFooter
        :total="totalRecords"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsModalOpen"
      :title="t('admin.sessions.ipBlacklist.detailTitle')"
      :description="t('admin.sessions.ipBlacklist.detailDescription')"
      max-width-class="sm:max-w-[700px] max-w-[calc(100vw-1rem)] p-4 sm:p-6"
      :loading="isDetailLoading"
      close-variant="outline"
    >
      <div v-if="detailRecord" class="space-y-4 overflow-x-auto">
        <div class="grid gap-3 md:grid-cols-2">
          <div
            class="border rounded-lg p-4 space-y-1"
            :class="detailRecord.ipLocation ? 'md:col-span-2' : ''"
          >
            <div class="text-sm text-muted-foreground">IP</div>
            <div class="font-mono text-base break-all">
              {{ detailRecord.ip }}
            </div>
            <div
              v-if="detailRecord.ipLocation"
              class="text-xs text-muted-foreground break-all"
            >
              {{ detailRecord.ipLocation }}
            </div>
          </div>

          <div class="border rounded-lg p-4 space-y-2">
            <div class="text-sm text-muted-foreground">
              {{ t("admin.sessions.ipBlacklist.blockedAt") }}
            </div>
            <div class="text-base break-all">
              {{ formatDate(detailRecord.blockedAt) }}
            </div>
          </div>

          <div class="border rounded-lg p-4 space-y-2">
            <div class="text-sm text-muted-foreground">
              {{ t("admin.sessions.ipBlacklist.triggerWindow") }}
            </div>
            <div class="text-base break-all">
              {{
                t("admin.sessions.ipBlacklist.minutes", {
                  count: detailRecord.windowMinutes,
                })
              }}
            </div>
          </div>

          <div class="border rounded-lg p-4 space-y-2">
            <div class="text-sm text-muted-foreground">
              {{ t("admin.sessions.ipBlacklist.triggerThreshold") }}
            </div>
            <div class="text-base break-all">
              {{
                t("admin.sessions.ipBlacklist.times", {
                  count: detailRecord.threshold,
                })
              }}
            </div>
          </div>
        </div>

        <BlacklistHitsTable :rows="detailHitRows" />
      </div>
    </DetailDialog>
  </div>
</template>
