<script setup lang="ts">
import { ref } from 'vue';
import { toast } from '@admin-shared/utils/toast';
import { SystemAPI } from '../../lib/api';
import { usePollingResourceStatus } from '@admin-shared/composables/usePollingResourceStatus';
import BinaryDownloadCard from '@admin-shared/components/system/BinaryDownloadCard.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';

const supported = ref(false);
const platform = ref<'darwin' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported'>('unsupported');
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
  fetcher: () => SystemAPI.getCloudflaredStatus(),
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
    const res = await SystemAPI.startCloudflaredDownload();
    if (res.success) {
      toast.success('开始下载 Cloudflared 资源');
      await fetchStatus();
      return;
    }
    toast.error(res.message || '启动下载失败');
  });
};

const deleteResource = async () => {
  await runDeleteResource(async () => {
    const res = await SystemAPI.deleteCloudflared();
    if (res.success) {
      toast.success('已删除 Cloudflared 资源');
      await fetchStatus();
      return;
    }
    toast.error(res.message || '删除失败');
  });
};

const cancelDownload = async () => {
  await runCancelDownload(async () => {
    const res = await SystemAPI.cancelCloudflaredDownload();
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
    title="Cloudflared资源"
    description="下载并管理 Cloudflared 可执行资源，便于使用 Cloudflare 内网穿透"
    :is-initializing="isInitializing"
    :supported="supported"
    :platform="platform"
    :downloaded="downloaded"
    :status="status"
    :percent="percent"
    :error="error"
    :is-cancelling="isCancelling"
    :allow-manage="platform === 'linux-amd64' || platform === 'linux-arm64' || platform === 'linux-arm'"
    ready-label="已就绪"
    pending-label="未就绪"
    download-button-text="下载资源"
    downloading-text="下载中，请稍候..."
    redownload-confirm-title="确认重新下载 Cloudflared 资源？"
    redownload-confirm-description="此操作会覆盖现有文件。"
    delete-confirm-title="确认删除 Cloudflared 资源？"
    delete-confirm-description="删除后需重新下载才能使用。"
    @start="startDownload"
    @cancel="cancelDownload"
    @redownload="startDownload"
    @delete="deleteResource"
  />
</template>
