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
const platform = ref<'darwin-arm64' | 'linux-amd64' | 'linux-arm64' | 'linux-arm' | 'unsupported'>('unsupported');
const downloaded = ref(false);
const status = ref<'idle' | 'downloading' | 'completed' | 'error'>('idle');
const percent = ref(0);
const error = ref('');
const { run: runStartDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.frpSettings.startDownloadFailed')));
  },
});
const { run: runDeleteResource } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.frpSettings.deleteFailed')));
  },
});
const { isPending: isCancelling, run: runCancelDownload } = useAsyncAction({
  onError: (error) => {
    toast.error(extractErrorMessage(error, t('admin.frpSettings.cancelFailed')));
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
      toast.success(t('admin.frpSettings.downloadStarted'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.frpSettings.startDownloadFailed'));
  });
};

const deleteResource = async () => {
  await runDeleteResource(async () => {
    const res = await SystemAPI.deleteFrp();
    if (res.success) {
      toast.success(t('admin.frpSettings.deleted'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.frpSettings.deleteFailed'));
  });
};

const cancelDownload = async () => {
  await runCancelDownload(async () => {
    const res = await SystemAPI.cancelFrpDownload();
    if (res.success) {
      toast.info(t('admin.frpSettings.cancelRequested'));
      await fetchStatus();
      return;
    }
    toast.error(res.message || t('admin.frpSettings.cancelFailed'));
  });
};

</script>

<template>
  <BinaryDownloadCard
    :title="t('admin.frpSettings.title')"
    :description="t('admin.frpSettings.description')"
    :is-initializing="isInitializing"
    :supported="supported"
    :platform="platform"
    :downloaded="downloaded"
    :status="status"
    :percent="percent"
    :error="error"
    :is-cancelling="isCancelling"
    :ready-label="t('admin.frpSettings.readyLabel')"
    :pending-label="t('admin.frpSettings.pendingLabel')"
    :download-button-text="t('admin.frpSettings.downloadButton')"
    :downloading-text="t('admin.frpSettings.downloading')"
    :redownload-confirm-title="t('admin.frpSettings.redownloadConfirmTitle')"
    :redownload-confirm-description="t('admin.frpSettings.redownloadConfirmDescription')"
    :delete-confirm-title="t('admin.frpSettings.deleteConfirmTitle')"
    :delete-confirm-description="t('admin.frpSettings.deleteConfirmDescription')"
    @start="startDownload"
    @cancel="cancelDownload"
    @redownload="startDownload"
    @delete="deleteResource"
  />
</template>
