<script setup lang="ts">
import { ref } from 'vue';
import { useI18n } from 'vue-i18n';
import { toast } from '@admin-shared/utils/toast';
import { SystemAPI } from '../../lib/api';
import { usePollingResourceStatus } from '@admin-shared/composables/usePollingResourceStatus';
import BinaryDownloadCard from '@admin-shared/components/system/BinaryDownloadCard.vue';
import { extractErrorMessage, useAsyncAction } from '@admin-shared/composables/useAsyncAction';

const { t } = useI18n();
const supported = ref(false);
const platform = ref<'darwin' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported'>('unsupported');
const downloaded = ref(false);
const status = ref<'idle' | 'downloading' | 'completed' | 'error'>('idle');
const percent = ref(0);
const error = ref('');
const { run: runStartDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.cloudflaredSettings.startDownloadFailed')));
  },
});
const { run: runDeleteResource } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.cloudflaredSettings.deleteFailed')));
  },
});
const { isPending: isCancelling, run: runCancelDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.cloudflaredSettings.cancelFailed')));
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
      toast.success(t('admin.cloudflaredSettings.downloadStarted'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.cloudflaredSettings.startDownloadFailed'));
  });
};

const deleteResource = async () => {
  await runDeleteResource(async () => {
    const res = await SystemAPI.deleteCloudflared();
    if (res.success) {
      toast.success(t('admin.cloudflaredSettings.deleted'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.cloudflaredSettings.deleteFailed'));
  });
};

const cancelDownload = async () => {
  await runCancelDownload(async () => {
    const res = await SystemAPI.cancelCloudflaredDownload();
    if (res.success) {
      toast.info(t('admin.cloudflaredSettings.cancelRequested'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.cloudflaredSettings.cancelFailed'));
  });
};

</script>

<template>
  <BinaryDownloadCard
    :title="t('admin.cloudflaredSettings.title')"
    :description="t('admin.cloudflaredSettings.description')"
    :is-initializing="isInitializing"
    :supported="supported"
    :platform="platform"
    :downloaded="downloaded"
    :status="status"
    :percent="percent"
    :error="error"
    :is-cancelling="isCancelling"
    :allow-manage="platform === 'linux-amd64' || platform === 'linux-arm64' || platform === 'linux-arm'"
    :ready-label="t('admin.cloudflaredSettings.readyLabel')"
    :pending-label="t('admin.cloudflaredSettings.pendingLabel')"
    :download-button-text="t('admin.cloudflaredSettings.downloadButton')"
    :downloading-text="t('admin.cloudflaredSettings.downloading')"
    :redownload-confirm-title="t('admin.cloudflaredSettings.redownloadConfirmTitle')"
    :redownload-confirm-description="t('admin.cloudflaredSettings.redownloadConfirmDescription')"
    :delete-confirm-title="t('admin.cloudflaredSettings.deleteConfirmTitle')"
    :delete-confirm-description="t('admin.cloudflaredSettings.deleteConfirmDescription')"
    @start="startDownload"
    @cancel="cancelDownload"
    @redownload="startDownload"
    @delete="deleteResource"
  />
</template>
