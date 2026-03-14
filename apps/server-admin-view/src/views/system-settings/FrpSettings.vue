<script setup lang="ts">
import { ref } from 'vue';
import { toast } from '@admin-shared/utils/toast';
import { SystemAPI } from '../../lib/api';
import { usePollingResourceStatus } from '@admin-shared/composables/usePollingResourceStatus';
import BinaryDownloadCard from '@admin-shared/components/system/BinaryDownloadCard.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';

const supported = ref(false);
const platform = ref<'darwin-arm64' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported'>('unsupported');
const downloaded = ref(false);
const status = ref<'idle' | 'downloading' | 'completed' | 'error'>('idle');
const percent = ref(0);
const error = ref('');
const { run: runStartDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '启动下载失败'));
  },
});
const { run: runDeleteResource } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '删除失败'));
  },
});
const { isPending: isCancelling, run: runCancelDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, '取消失败'));
  },
});
const { isInitializing, refresh: fetchStatus } = usePollingResourceStatus({
  fetcher: () => SystemAPI.getFrpStatus(),
  onData: (res) => {
    if (!res.success || !res.data) return;
    supported.value = res.data.supported;
    platform.value = res.data.platform;
    downloaded.value = res.data.downloaded;
    status.value = res.data.progress?.status || 'idle';
    percent.value = res.data.progress?.percent || 0;
    error.value = res.data.progress?.error || '';
  },
  isDownloading: (res) => Boolean(res.success && res.data?.progress?.status === 'downloading'),
});

const startDownload = async () => {
  await runStartDownload(async () => {
    error.value = '';
    const res = await SystemAPI.startFrpDownload();
    if (res.success) {
      toast.success('开始下载 FRP 资源');
      await fetchStatus();
      return;
    }
    toast.error(res.message || '启动下载失败');
  });
};

const deleteResource = async () => {
  await runDeleteResource(async () => {
    const res = await SystemAPI.deleteFrp();
    if (res.success) {
      toast.success('已删除 FRP 资源');
      await fetchStatus();
      return;
    }
    toast.error(res.message || '删除失败');
  });
};

const cancelDownload = async () => {
  await runCancelDownload(async () => {
    const res = await SystemAPI.cancelFrpDownload();
    if (res.success) {
      toast.info('已请求取消下载');
      await fetchStatus();
      return;
    }
    toast.error(res.message || '取消失败');
  });
};

</script>

<template>
  <BinaryDownloadCard
    title="FRP FRP穿透"
    description="下载并管理 FRP 可执行资源，便于内网穿透使用"
    :is-initializing="isInitializing"
    :supported="supported"
    :platform="platform"
    :downloaded="downloaded"
    :status="status"
    :percent="percent"
    :error="error"
    :is-cancelling="isCancelling"
    ready-label="已下载"
    pending-label="未下载"
    download-button-text="下载 FRP 资源"
    downloading-text="下载中，请稍候..."
    redownload-confirm-title="确认重新下载 FRP 资源？"
    redownload-confirm-description="此操作会覆盖现有文件。"
    delete-confirm-title="确认删除 FRP 资源？"
    delete-confirm-description="删除后需重新下载才能使用。"
    @start="startDownload"
    @cancel="cancelDownload"
    @redownload="startDownload"
    @delete="deleteResource"
  />
</template>
