<script setup lang="ts">
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import { Button } from '@/components/ui/button';
import RefreshButton from '@/components/RefreshButton.vue';
import SearchInput from '@admin-shared/components/SearchInput.vue';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Checkbox } from '@/components/ui/checkbox';
import { Badge } from '@/components/ui/badge';
import { toast } from '@admin-shared/utils/toast';
import { Ban, Eye, Loader2, Settings, Trash2 } from 'lucide-vue-next';
import VChart from 'vue-echarts';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart } from 'echarts/charts';
import { GridComponent, TooltipComponent } from 'echarts/components';
import { ScannerAPI, SecurityAPI, type ScannerBlacklistRecord } from '../../lib/api';
import { DEFAULT_THREAT_RANGES, useThreatOverview } from '@admin-shared/composables/useThreatOverview';
import { usePagedSelectionList } from '@admin-shared/composables/usePagedSelectionList';
import ThreatOverviewCard from '@admin-shared/components/common/ThreatOverviewCard.vue';
import PagedTableFooter from '@admin-shared/components/list/PagedTableFooter.vue';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import DetailDialog from '@admin-shared/components/common/DetailDialog.vue';
import TableSkeletonBlock from '@admin-shared/components/list/TableSkeletonBlock.vue';
import DetailFieldsGrid from '@admin-shared/components/common/DetailFieldsGrid.vue';
import BlacklistHitsTable from '@admin-shared/components/session/BlacklistHitsTable.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { buildDetailFields } from '@admin-shared/utils/buildDetailFields';
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe';
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue';

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent]);

const ranges = DEFAULT_THREAT_RANGES;

const {
  rangeKey,
  threatOverview,
  isThreatLoading,
  titleRangeText,
  perHour: blockedPerHour,
  formatNumber,
  formatRate,
  trendOption: blockedTrendOption,
  fetchThreatOverview,
} = useThreatOverview({
  defaultRangeKey: '1h',
  ranges,
  seriesKey: 'blockedScanners',
  seriesName: '拦截扫描器',
  lineColor: '#f97316',
  areaStartColor: 'rgba(249, 115, 22, 0.18)',
  areaEndColor: 'rgba(249, 115, 22, 0)',
  fetchOverview: (rangeSec) => SecurityAPI.getOverview(rangeSec),
  onError: (err: any) => {
    const msg = err?.response?.data?.message || err?.message || '加载失败';
    toast.error('威胁态势加载失败', { description: msg });
  },
});

const router = useRouter();
const { isPending: isDeleting, run: runDeleteAction } = useAsyncAction({
  onError: (error) => {
    toast.error('删除失败', { description: extractErrorMessage(error, '删除失败') });
  },
});
const { isPending: isDetailLoading, run: runLoadDetail } = useAsyncAction({
  onError: (error) => {
    toast.error('加载失败', { description: extractErrorMessage(error, '加载详情失败') });
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
    const msg = err?.response?.data?.message || err?.message || '加载失败';
    toast.error('加载失败', { description: msg });
  },
});

const isDetailsModalOpen = ref(false);
const detailRecord = ref<ScannerBlacklistRecord | null>(null);
const showTableSkeleton = useDelayedLoading(() => loading.value && records.value.length === 0);

const deleteBlacklist = async (ips: string[]) => {
  if (ips.length === 0) return;
  await runDeleteAction(
    () => ScannerAPI.deleteBlacklist(ips),
    {
      onSuccess: async () => {
        toast.success('删除成功');
        clearSelection();
        await fetchBlacklist();
      },
    },
  );
};

const deleteOne = async (ip: string) => {
  await runDeleteAction(
    () => ScannerAPI.deleteBlacklistByIp(ip),
    {
      onSuccess: async () => {
        toast.success('删除成功');
        selectedIps.value.delete(ip);
        selectedIps.value = new Set(selectedIps.value);
        await fetchBlacklist();
      },
    },
  );
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
  return formatDateTimeSafe(ts);
};

const formatIntervalMinutes = (value: number | null) => {
  if (value === null) return '-';
  if (!Number.isFinite(value)) return '-';
  return `${value.toFixed(2)} 分钟`;
};

const detailHits = computed(() => {
  if (!detailRecord.value?.hits) return [];
  const sorted = [...detailRecord.value.hits].sort((a, b) => a.createdAt - b.createdAt);
  return sorted.map((hit, index) => {
    const prev = sorted[index - 1];
    const intervalMinutes = prev ? (hit.createdAt - prev.createdAt) / 60000 : null;
    return { ...hit, intervalMinutes };
  });
});

const detailHitRows = computed(() =>
  detailHits.value.map((hit, index) => ({
    key: `${hit.createdAt}-${index}`,
    time: formatDate(hit.createdAt),
    path: hit.path,
    interval: formatIntervalMinutes(hit.intervalMinutes),
  })),
);

const detailMetaDefinitions = [
  { key: 'ip', label: 'IP' },
  { key: 'blockedAt', label: '封禁时间' },
  { key: 'windowMinutes', label: '触发窗口' },
  { key: 'threshold', label: '触发阈值' },
] as const;

const detailMetaItems = computed(() =>
  buildDetailFields(detailRecord.value, detailMetaDefinitions, {
    format: (key, value) => {
      if (key === 'blockedAt') return formatDate(value);
      if (key === 'windowMinutes') return `${value} 分钟`;
      if (key === 'threshold') return `${value} 次`;
      return value;
    },
  }),
);

onMounted(() => {
  fetchBlacklist();
  fetchThreatOverview();
});

const goToFirewallSettings = () => {
  router.push({ path: '/system', query: { tab: 'scanner-firewall' } });
};
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <ConfigCollapsibleCard
      title="扫描拦截图表"
      :configured="true"
      edit-label="展开图表"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{ titleRangeText }} · 拦截扫描器 {{ formatNumber(threatOverview?.totals?.blockedScanners) }}
      </template>

      <template #default>
        <ThreatOverviewCard
          v-model:range-key="rangeKey"
          title="扫描拦截"
          description="黑名单趋势"
          :ranges="ranges"
          :is-loading="isThreatLoading"
          :title-range-text="titleRangeText"
          primary-label="拦截扫描器"
          :primary-value="formatNumber(threatOverview?.totals?.blockedScanners)"
          primary-hint="新增拉黑 IP"
          secondary-label="平均每小时"
          :secondary-value="formatRate(blockedPerHour)"
          secondary-hint="按范围计算"
          :icon="Ban"
        >
          <template #chart>
            <VChart :option="blockedTrendOption" class="h-full w-full" />
          </template>
        </ThreatOverviewCard>
      </template>
    </ConfigCollapsibleCard>

    <div class="flex items-center gap-2">
      <SearchInput
        v-model="searchQuery"
        placeholder="搜索 IP..."
        class="w-[260px]"
        @search="handleSearch"
      />
      <div class="flex-1"></div>
      <RefreshButton :loading="loading" :disabled="loading" @click="fetchBlacklist" />
      <Button variant="outline" @click="goToFirewallSettings">
        <Settings class="h-4" />
        设置
      </Button>
      <ConfirmDangerPopover
        :title="`确认删除 ${selectedIps.size} 个 IP？`"
        description="删除后将立即解除拉黑。"
        :loading="isDeleting"
        :disabled="selectedIps.size === 0 || isDeleting"
        :on-confirm="() => deleteBlacklist(Array.from(selectedIps))"
      >
        <template #trigger>
          <Button variant="destructive" :disabled="selectedIps.size === 0 || isDeleting">
            <Trash2 class="h-4" />
            删除已选 ({{ selectedIps.size }})
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>

    <div class="border rounded-md overflow-hidden bg-background flex-1 flex flex-col">
      <div class="flex-1 w-full overflow-hidden">
        <div class="h-full overflow-auto">
          <Table v-if="!(loading && records.length === 0)">
            <TableHeader class="sticky top-0 bg-background z-10 shadow-sm">
              <TableRow>
                <TableHead class="w-[50px]">
                  <Checkbox v-model="isAllSelected" />
                </TableHead>
                <TableHead>IP</TableHead>
                <TableHead>封禁时间</TableHead>
                <TableHead>窗口</TableHead>
                <TableHead>阈值</TableHead>
                <TableHead>命中</TableHead>
                <TableHead class="text-right pr-6">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="7" class="text-center py-10">
                  <Loader2 class="h-6 w-6 animate-spin mx-auto text-muted-foreground" />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="records.length === 0">
                <TableCell colspan="7" class="text-center py-10 text-muted-foreground">
                  暂无黑名单记录
                </TableCell>
              </TableRow>
              <TableRow v-else v-for="record in records" :key="record.ip">
                <TableCell>
                  <Checkbox
                    :model-value="selectedIps.has(record.ip)"
                    @update:model-value="toggleSelect(record.ip)"
                  />
                </TableCell>
                <TableCell class="font-mono text-sm">{{ record.ip }}</TableCell>
                <TableCell class="whitespace-nowrap"><HumanFriendlyTime :value="record.blockedAt" /></TableCell>
                <TableCell>{{ record.windowMinutes }} 分钟</TableCell>
                <TableCell>{{ record.threshold }} 次</TableCell>
                <TableCell>
                  <Badge variant="secondary">{{ record.hits?.length || 0 }}</Badge>
                </TableCell>
                <TableCell class="text-right space-x-2 pr-6">
                  <Button variant="ghost" size="icon" @click="viewDetails(record)">
                    <Eye class="h-4 w-4" />
                  </Button>
                  <ConfirmDangerPopover
                    title="确认删除该 IP？"
                    description="删除后将立即解除拉黑。"
                    :loading="isDeleting"
                    :disabled="isDeleting"
                    :on-confirm="() => deleteOne(record.ip)"
                  >
                    <template #trigger>
                      <Button variant="ghost" size="icon" class="text-destructive" :disabled="isDeleting">
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
            :header-widths="['w-4', 'w-24', 'w-20', 'w-10', 'w-10', 'w-10', 'w-10']"
            :row-widths="['w-4', 'w-24', 'w-20', 'w-10', 'w-10', 'w-10', 'w-16']"
          />
          <div v-else class="h-[380px]" aria-hidden="true" ></div>
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
      title="详情"
      description="查看该 IP 的封禁原因与访问路径。"
      max-width-class="sm:max-w-[700px]"
      :loading="isDetailLoading"
      close-variant="outline"
    >
      <div v-if="detailRecord" class="space-y-4">
        <DetailFieldsGrid layout="card" :items="detailMetaItems" />

        <BlacklistHitsTable :rows="detailHitRows" />
      </div>
    </DetailDialog>
  </div>
</template>
