<script setup lang="ts">
import { computed, ref, onMounted } from 'vue';
import { Button } from '@/components/ui/button';
import RefreshButton from '@/components/RefreshButton.vue';
import SearchInput from '@admin-shared/components/SearchInput.vue';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import { Checkbox } from '@/components/ui/checkbox';
import { toast } from '@admin-shared/utils/toast';
import { Eye, Loader2, ShieldAlert, Trash2 } from 'lucide-vue-next';
import VChart from 'vue-echarts';
import { use } from 'echarts/core';
import { CanvasRenderer } from 'echarts/renderers';
import { LineChart } from 'echarts/charts';
import { GridComponent, TooltipComponent } from 'echarts/components';
import { AuthLogsAPI, SecurityAPI } from '../lib/api';
import { DEFAULT_THREAT_RANGES, useThreatOverview } from '@admin-shared/composables/useThreatOverview';
import { usePagedSelectionList } from '@admin-shared/composables/usePagedSelectionList';
import ThreatOverviewCard from '@admin-shared/components/common/ThreatOverviewCard.vue';
import PagedTableFooter from '@admin-shared/components/list/PagedTableFooter.vue';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';
import DetailDialog from '@admin-shared/components/common/DetailDialog.vue';
import TableSkeletonBlock from '@admin-shared/components/list/TableSkeletonBlock.vue';
import DetailFieldsGrid from '@admin-shared/components/common/DetailFieldsGrid.vue';
import HumanFriendlyTime from '@admin-shared/components/common/HumanFriendlyTime.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import { useDelayedLoading } from '@admin-shared/composables/useDelayedLoading';
import { buildDetailFields } from '@admin-shared/utils/buildDetailFields';
import { formatDateTimeSafe } from '@admin-shared/utils/formatDateTimeSafe';
import ConfigCollapsibleCard from '@admin-shared/components/ConfigCollapsibleCard.vue';

use([CanvasRenderer, LineChart, GridComponent, TooltipComponent]);

interface AuthLog {
  id: string;
  type: 'login' | 'logout';
  method?: 'TOTP' | 'PASSKEY';
  credentialName?: string;
  time: string;
  ip: string;
  userAgent: string;
  success: boolean;
  message?: string;
  ipLocation?: string;
}

const ranges = DEFAULT_THREAT_RANGES;

const {
  rangeKey,
  threatOverview,
  isThreatLoading,
  titleRangeText,
  perHour: failedPerHour,
  formatNumber,
  formatRate,
  trendOption: failedTrendOption,
  fetchThreatOverview,
} = useThreatOverview({
  defaultRangeKey: '1h',
  ranges,
  seriesKey: 'failedLogins',
  seriesName: '登录失败',
  lineColor: '#ef4444',
  areaStartColor: 'rgba(239, 68, 68, 0.18)',
  areaEndColor: 'rgba(239, 68, 68, 0)',
  fetchOverview: (rangeSec) => SecurityAPI.getOverview(rangeSec),
  onError: (err: any) => {
    const msg = err?.response?.data?.message || err?.message || '加载失败';
    toast.error('威胁态势加载失败', { description: msg });
  },
});

const { isPending: isDeleting, run: runDeleteLogs } = useAsyncAction({
  onError: (error) => {
    toast.error('删除出错', { description: extractErrorMessage(error, '删除失败') });
  },
});

const {
  items: logs,
  total: totalLogs,
  loading,
  searchQuery,
  currentPage,
  limit,
  parsedLimit,
  selectedKeys: selectedIds,
  isAllSelected,
  fetchList: fetchLogs,
  handleSearch,
  handlePageChange,
  handleLimitChange,
  toggleSelect,
  clearSelection,
} = usePagedSelectionList<AuthLog, string>({
  fetchPage: async ({ page, limit, query }) => {
    const data = await AuthLogsAPI.getLogs(page, limit, query);
    if (!(data.success || data.data)) {
      throw new Error(data.message || '加载失败');
    }
    return {
      items: data.data.logs || [],
      total: data.data.total || 0,
    };
  },
  getKey: (log) => log.id,
  onError: (err: any) => {
    const msg = err?.response?.data?.message || err?.message || '加载失败';
    toast.error('加载失败', { description: msg });
  },
});

const isDetailsModalOpen = ref(false);
const activeLogDetails = ref<AuthLog | null>(null);
const showTableSkeleton = useDelayedLoading(() => loading.value && logs.value.length === 0);

const deleteLogs = async (ids: string[]) => {
  await runDeleteLogs(
    () => AuthLogsAPI.deleteLogs(ids),
    {
      onSuccess: async (data) => {
        if (data.success || data.message === 'success') {
          toast.success('删除成功');
          clearSelection();
          await fetchLogs();
          return;
        }
        toast.error('删除失败', { description: data.message });
      },
    },
  );
};

const viewDetails = (log: AuthLog) => {
  activeLogDetails.value = log;
  isDetailsModalOpen.value = true;
};

const formatDate = (dateStr: string) => formatDateTimeSafe(dateStr);

const fieldDefinitions = [
  { key: 'id', label: '日志 ID' },
  { key: 'type', label: '操作类型' },
  { key: 'method', label: '鉴权方式' },
  { key: 'credentialName', label: '凭证名称' },
  { key: 'time', label: '时间' },
  { key: 'ip', label: 'IP' },
  { key: 'ipLocation', label: '归属地' },
  { key: 'userAgent', label: 'UserAgent' },
  { key: 'success', label: '状态' },
  { key: 'message', label: '详情消息' },
] as const;

const formatDetailValue = (key: string, value: any) => {
  if (key === 'time') return formatDate(value);
  if (key === 'type') return value === 'login' ? '登录' : '退出';
  if (key === 'success') return value ? '成功' : '失败';
  if (value === undefined || value === null || value === '') return '-';
  return value;
};

const detailItems = computed(() =>
  buildDetailFields(activeLogDetails.value, fieldDefinitions, {
    format: (key, value) => formatDetailValue(key, value),
  }),
);

onMounted(() => {
  fetchLogs();
  fetchThreatOverview();
});
</script>

<template>
  <div class="h-full flex flex-col gap-4">
    <ConfigCollapsibleCard
      title="异常登录图表"
      :configured="true"
      edit-label="展开图表"
      summary-class="text-xs text-muted-foreground"
      expanded-content-class="p-0 sm:p-0"
    >
      <template #summary>
        {{ titleRangeText }} · 登录失败 {{ formatNumber(threatOverview?.totals?.failedLogins) }}
      </template>

      <template #default>
        <ThreatOverviewCard
          v-model:range-key="rangeKey"
          title="异常登录"
          description="登录失败趋势"
          :ranges="ranges"
          :is-loading="isThreatLoading"
          :title-range-text="titleRangeText"
          primary-label="登录失败量"
          :primary-value="formatNumber(threatOverview?.totals?.failedLogins)"
          primary-hint="所选时间范围"
          secondary-label="平均每小时"
          :secondary-value="formatRate(failedPerHour)"
          secondary-hint="按范围计算"
          :icon="ShieldAlert"
        >
          <template #chart>
            <VChart :option="failedTrendOption" class="h-full w-full" />
          </template>
        </ThreatOverviewCard>
      </template>
    </ConfigCollapsibleCard>

    <!-- Toolbar -->
    <div class="flex items-center gap-2">
      <SearchInput
        v-model="searchQuery"
        placeholder="搜索 ID、IP、凭证名、浏览器..."
        class="w-[320px]"
        @search="handleSearch"
      />
      <div class="flex-1"></div>
      <RefreshButton :loading="loading" :disabled="loading" @click="fetchLogs" />
      <ConfirmDangerPopover
        :title="`确认删除 ${selectedIds.size} 条日志？`"
        description="删除后记录将无法恢复。"
        :loading="isDeleting"
        :disabled="selectedIds.size === 0 || isDeleting"
        :on-confirm="() => deleteLogs(Array.from(selectedIds))"
      >
        <template #trigger>
          <Button 
            variant="destructive" 
            :disabled="selectedIds.size === 0 || isDeleting"
          >
            <Trash2 class="mr-2 h-4 w-4" />
            删除已选 ({{ selectedIds.size }})
          </Button>
        </template>
      </ConfirmDangerPopover>
    </div>

    <!-- Table -->
    <div class="border rounded-md overflow-hidden bg-background flex-1 flex flex-col">
      <div class="flex-1 w-full overflow-hidden">
        <div class="h-full overflow-auto">
          <Table v-if="!(loading && logs.length === 0)">
            <TableHeader class="sticky top-0 bg-background z-10 shadow-sm">
              <TableRow>
                <TableHead class="w-[50px]">
                  <Checkbox 
                    v-model="isAllSelected"
                  />
                </TableHead>
                <TableHead>时间</TableHead>
                <TableHead>类型</TableHead>
                <TableHead>方式</TableHead>
                <TableHead>凭证名</TableHead>
                <TableHead>IP / 归属地</TableHead>
                <TableHead>User-Agent</TableHead>
                <TableHead class="text-right pr-6">操作</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              <TableRow v-if="loading">
                <TableCell colspan="8" class="text-center py-10">
                  <Loader2 class="h-6 w-6 animate-spin mx-auto text-muted-foreground" />
                </TableCell>
              </TableRow>
              <TableRow v-else-if="logs.length === 0">
                <TableCell colspan="8" class="text-center py-10 text-muted-foreground">
                  暂无日志
                </TableCell>
              </TableRow>
              <TableRow v-else v-for="log in logs" :key="log.id">
                <TableCell>
                  <Checkbox 
                    :model-value="selectedIds.has(log.id)"
                    @update:model-value="toggleSelect(log.id)"
                  />
                </TableCell>
                <TableCell class="whitespace-nowrap"><HumanFriendlyTime :value="log.time" /></TableCell>
                <TableCell>
                  <div class="flex items-center gap-2">
                    <div :class="['w-2 h-2 rounded-full', (log.type === 'login' && log.success) ? 'bg-green-500' : 'bg-red-500']"></div>
                    {{ log.type === 'login' ? (log.success ? '登录' : '登录失败') : '退出' }}
                  </div>
                </TableCell>
                <TableCell>{{ log.method || '-' }}</TableCell>
                <TableCell>{{ log.credentialName || '-' }}</TableCell>
                <TableCell>
                  <div>{{ log.ip }}</div>
                  <div v-if="log.ipLocation" class="text-xs text-muted-foreground mt-0.5">{{ log.ipLocation }}</div>
                </TableCell>
                <TableCell class="max-w-[200px] truncate" :title="log.userAgent">{{ log.userAgent }}</TableCell>
                <TableCell class="text-right space-x-2 pr-6">
                  <Button variant="ghost" size="icon" @click="viewDetails(log)">
                    <Eye class="h-4 w-4" />
                  </Button>
                  <ConfirmDangerPopover
                    title="确认删除该日志？"
                    description="删除后记录将无法恢复。"
                    :loading="isDeleting"
                    :disabled="isDeleting"
                    :on-confirm="() => deleteLogs([log.id])"
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
            :header-widths="['w-4', 'w-16', 'w-10', 'w-12', 'w-16', 'w-24', 'w-28', 'w-10']"
            :row-widths="['w-4', 'w-28', 'w-10', 'w-16', 'w-20', 'w-40', 'w-40', 'w-16']"
          />
          <div v-else class="h-[380px]" aria-hidden="true" ></div>
        </div>
      </div>
      
      <PagedTableFooter
        :total="totalLogs"
        :page="currentPage"
        :limit="limit"
        :items-per-page="parsedLimit"
        @update:page="handlePageChange"
        @update:limit="handleLimitChange"
      />
    </div>

    <DetailDialog
      v-model:open="isDetailsModalOpen"
      title="日志详情"
      description="查看此条日志的完整详细信息。"
      max-width-class="sm:max-w-[500px]"
      close-variant="default"
    >
      <div class="py-4 max-h-[60vh] overflow-y-auto" v-if="activeLogDetails">
        <DetailFieldsGrid :items="detailItems" />
      </div>
    </DetailDialog>
  </div>
</template>
