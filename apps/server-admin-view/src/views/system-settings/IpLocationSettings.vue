<script setup lang="ts">
import { ref } from 'vue';
import { Button } from '@/components/ui/button';
import { Progress } from '@/components/ui/progress';
import { Skeleton } from '@/components/ui/skeleton';
import { toast } from '@admin-shared/utils/toast';
import { SystemAPI } from '../../lib/api';
import { usePollingResourceStatus } from '@admin-shared/composables/usePollingResourceStatus';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';
import ResourceStatusCard from '@admin-shared/components/system/ResourceStatusCard.vue';
import ConfirmDangerPopover from '@admin-shared/components/common/ConfirmDangerPopover.vue';

const isInitialized = ref(false);
const hasV4 = ref(false);
const hasV6 = ref(false);
const downloadStatus = ref('idle');
const progressV4 = ref(0);
const progressV6 = ref(0);
const downloadError = ref('');
const { isPending: isStarting, run: runStartDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '启动下载失败'));
  },
});
const { isPending: isCancelling, run: runCancelDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '取消下载失败'));
  },
});
const { isInitializing, refresh: fetchStatus } = usePollingResourceStatus({
  fetcher: () => SystemAPI.getIpLocationStatus(),
  onData: (res) => {
    if (!res.success || !res.data) return;
    isInitialized.value = res.data.isInitialized;
    hasV4.value = res.data.hasV4;
    hasV6.value = res.data.hasV6;
    downloadStatus.value = res.data.progress?.status || 'idle';
    progressV4.value = res.data.progress?.progressV4 || 0;
    progressV6.value = res.data.progress?.progressV6 || 0;
    downloadError.value = res.data.progress?.error || '';
  },
  isDownloading: (res) => Boolean(res.success && res.data?.progress?.status === 'downloading'),
});

const startDownload = async () => {
  await runStartDownload(async () => {
    downloadError.value = '';
    const res = await SystemAPI.startIpLocationDownload();
    if (res.success) {
      toast.success('开始下载 IP 数据库');
      fetchStatus();
    } else {
      toast.error(res.message || '启动下载失败');
    }
  });
};

const cancelDownload = async () => {
  await runCancelDownload(async () => {
    const res = await SystemAPI.cancelIpLocationDownload();
    if (res.success) {
      toast.info('已请求取消下载');
      fetchStatus();
    }
  });
};

</script>

<template>
  <ResourceStatusCard
    title="IP 归属地数据库"
    description="用于解析白名单和日志中的 IP 归属地"
    :is-initializing="isInitializing"
  >
    <template #initial>
      <div class="flex items-center gap-4">
        <div class="flex-1 border p-4 rounded-lg">
          <Skeleton class="h-5 w-28" />
          <div class="mt-3 space-y-2">
            <Skeleton class="h-4 w-[60%]" />
            <Skeleton class="h-4 w-[50%]" />
          </div>
        </div>
      </div>
      <div class="grid gap-4 bg-muted/30 p-4 rounded-lg border">
        <div class="space-y-2">
          <Skeleton class="h-4 w-40" />
          <Skeleton class="h-3 w-full" />
        </div>
        <div class="space-y-2">
          <Skeleton class="h-4 w-44" />
          <Skeleton class="h-3 w-full" />
        </div>
      </div>
    </template>

    <div class="flex items-center gap-4">
      <div class="flex-1 border p-4 rounded-lg">
        <div class="font-medium flex justify-between">
          <span>初始化状态</span>
          <span :class="['px-2 py-0.5 rounded text-xs font-medium', isInitialized ? 'bg-green-100 text-green-700 border border-green-200' : 'bg-yellow-100 text-yellow-700 border border-yellow-200']">
            {{ isInitialized ? '已初始化' : '未初始化' }}
          </span>
        </div>
        <div class="text-sm text-muted-foreground mt-3 flex flex-col gap-1">
          <div class="flex justify-between">
            <span>IPv4归属地:</span>
            <span :class="hasV4 ? 'text-green-600' : 'text-red-500'">{{ hasV4 ? '已就绪' : '缺失' }}</span>
          </div>
          <div class="flex justify-between">
            <span>IPv6归属地:</span>
            <span :class="hasV6 ? 'text-green-600' : 'text-red-500'">{{ hasV6 ? '已就绪' : '缺失' }}</span>
          </div>
        </div>
      </div>
    </div>

    <div v-if="downloadStatus === 'downloading'" class="grid gap-4 bg-muted/30 p-4 rounded-lg border">
      <div>
        <div class="flex justify-between text-sm mb-2 text-muted-foreground font-medium">
          <span>IPv4 数据库下载进度</span>
          <span>{{ progressV4 }}%</span>
        </div>
        <Progress :model-value="progressV4" />
      </div>
      <div>
        <div class="flex justify-between text-sm mb-2 text-muted-foreground font-medium">
          <span>IPv6 数据库下载进度</span>
          <span>{{ progressV6 }}%</span>
        </div>
        <Progress :model-value="progressV6" />
      </div>
    </div>

    <div v-if="downloadError" class="text-sm bg-destructive/10 text-destructive p-3 rounded-md border border-destructive/20 mt-2">
      错误：{{ downloadError }}
    </div>

    <template #footer>
      <template v-if="downloadStatus !== 'downloading'">
        <template v-if="!isInitialized">
          <Button @click="startDownload" :disabled="isStarting">
            下载并初始化
          </Button>
        </template>
        <template v-else>
          <ConfirmDangerPopover
            title="确认重新下载 IP 数据库？"
            description="此操作会覆盖现有数据文件。"
            confirm-text="确认重新下载"
            confirm-variant="default"
            :loading="isStarting"
            :disabled="isStarting"
            :on-confirm="startDownload"
          >
            <template #trigger>
              <Button variant="outline" :disabled="isStarting">
                重新下载IP数据库
              </Button>
            </template>
          </ConfirmDangerPopover>
        </template>
      </template>
      <template v-else>
        <div class="text-sm text-muted-foreground animate-pulse flex items-center h-10 mr-auto">下载中，请随时关注进度...</div>
        <Button variant="destructive" @click="cancelDownload" :disabled="isCancelling">取消任务</Button>
      </template>
    </template>
  </ResourceStatusCard>
</template>
