import { defineStore } from 'pinia';
import { computed, ref } from 'vue';
import { UpdateAPI, type UpdateStatusPayload } from '../lib/api';
import { toast } from '@admin-shared/utils/toast';
import { extractErrorMessage } from '@admin-shared/composables/useAsyncAction';

const POLL_IDLE_MS = 15_000;
const POLL_BUSY_MS = 1_000;
const INSTALL_PREPARE_MS = 4_000;

export const useUpdateStore = defineStore('update', () => {
  const status = ref<UpdateStatusPayload | null>(null);
  const isLoading = ref(false);
  const isChecking = ref(false);
  const isTriggeringDownload = ref(false);
  const isTriggeringInstall = ref(false);
  const isPreparingInstall = ref(false);
  const shouldAutoInstallAfterDownload = ref(false);
  const hasInitialized = ref(false);
  let pollTimer: number | null = null;
  let pollingStarted = false;

  const downloadStatus = computed(() => status.value?.download.status ?? 'idle');
  const isDownloadBusy = computed(() =>
    downloadStatus.value === 'downloading'
    || downloadStatus.value === 'verifying'
    || downloadStatus.value === 'installing',
  );
  const canInstall = computed(() => {
    return status.value?.hasUpdate === true && status.value.download.status === 'downloaded';
  });
  const shouldShowBanner = computed(() => status.value?.hasUpdate === true);
  const isForceUpdate = computed(() => status.value?.forceUpdate === true && shouldShowBanner.value);

  const clearTimer = () => {
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const schedulePoll = () => {
    clearTimer();
    if (!pollingStarted) return;
    pollTimer = window.setTimeout(async () => {
      if (!isPreparingInstall.value) {
        await loadStatus(true);
        await maybeAutoInstall();
      }
      schedulePoll();
    }, isDownloadBusy.value ? POLL_BUSY_MS : POLL_IDLE_MS);
  };

  async function maybeAutoInstall() {
    if (!shouldAutoInstallAfterDownload.value) return;
    if (isTriggeringInstall.value) return;
    if (!canInstall.value) return;
    shouldAutoInstallAfterDownload.value = false;
    await startInstall();
  }

  async function loadStatus(silent = false) {
    if (!silent) isLoading.value = true;
    try {
      status.value = await UpdateAPI.getStatus();
    } catch (error) {
      if (!silent) {
        toast.error('加载更新状态失败', {
          description: extractErrorMessage(error, '请稍后重试'),
        });
      }
    } finally {
      if (!silent) isLoading.value = false;
    }
  }

  async function checkNow(showToast = true) {
    if (isChecking.value) return false;
    isChecking.value = true;
    try {
      status.value = await UpdateAPI.checkNow();
      if (showToast) {
        if (status.value.hasUpdate) {
          toast.success(`检测到新版本 ${status.value.latest?.version || ''}`);
        } else if (status.value.updateEnabled) {
          toast.success('当前已是最新版本');
        } else {
          toast.info('更新功能暂未启用');
        }
      }
      return true;
    } catch (error) {
      if (showToast) {
        toast.error('检查更新失败', {
          description: extractErrorMessage(error, '请稍后重试'),
        });
      }
      return false;
    } finally {
      isChecking.value = false;
      schedulePoll();
    }
  }

  async function checkAndDownload() {
    if (isTriggeringDownload.value) return false;
    isTriggeringDownload.value = true;
    shouldAutoInstallAfterDownload.value = false;
    try {
      const res = await UpdateAPI.checkAndDownload();
      if (!res.success) {
        toast.error('启动更新失败', { description: res.message || '请稍后重试' });
        return false;
      }
      if (res.data) {
        status.value = res.data;
      }
      await loadStatus(true);
      schedulePoll();
      if (status.value?.hasUpdate) {
        shouldAutoInstallAfterDownload.value = true;
        await maybeAutoInstall();
      }
      toast.success(res.message || '已开始下载更新包');
      return true;
    } catch (error) {
      toast.error('启动更新失败', {
        description: extractErrorMessage(error, '请稍后重试'),
      });
      return false;
    } finally {
      isTriggeringDownload.value = false;
    }
  }

  async function startDownload() {
    if (isTriggeringDownload.value) return false;
    isTriggeringDownload.value = true;
    try {
      const res = await UpdateAPI.startDownload();
      if (!res.success) {
        toast.error('启动下载失败', { description: res.message || '请稍后重试' });
        return false;
      }
      if (res.data) {
        status.value = res.data;
      }
      await loadStatus(true);
      schedulePoll();
      toast.success(res.message || '已开始下载更新包');
      return true;
    } catch (error) {
      toast.error('启动下载失败', {
        description: extractErrorMessage(error, '请稍后重试'),
      });
      return false;
    } finally {
      isTriggeringDownload.value = false;
    }
  }

  async function startInstall() {
    if (isTriggeringInstall.value) return false;
    shouldAutoInstallAfterDownload.value = false;
    isTriggeringInstall.value = true;
    isPreparingInstall.value = true;
    try {
      if (status.value) {
        status.value.download.status = 'installing';
        status.value.download.error = null;
      }
      schedulePoll();
      await new Promise((resolve) => window.setTimeout(resolve, INSTALL_PREPARE_MS));

      const res = await UpdateAPI.startInstall();
      if (!res.success) {
        toast.error('启动安装失败', { description: res.message || '请稍后重试' });
        await loadStatus(true);
        schedulePoll();
        return false;
      }
      await loadStatus(true);
      schedulePoll();
      return true;
    } catch (error) {
      toast.error('启动安装失败', {
        description: extractErrorMessage(error, '请稍后重试'),
      });
      await loadStatus(true);
      schedulePoll();
      return false;
    } finally {
      isPreparingInstall.value = false;
      isTriggeringInstall.value = false;
    }
  }

  async function consumeConfirm() {
    try {
      const confirm = await UpdateAPI.consumeConfirm();
      if (!confirm?.version) return;
      toast.success(`更新完成到 ${confirm.version}`);
    } catch {
      // ignore confirm errors
    }
  }

  async function initialize() {
    if (hasInitialized.value) return;
    hasInitialized.value = true;
    await Promise.all([loadStatus(true), consumeConfirm()]);
    startPolling();
  }

  function startPolling() {
    if (pollingStarted) return;
    pollingStarted = true;
    schedulePoll();
  }

  function stopPolling() {
    pollingStarted = false;
    clearTimer();
  }

  return {
    status,
    isLoading,
    isChecking,
    isTriggeringDownload,
    isTriggeringInstall,
    canInstall,
    shouldShowBanner,
    isForceUpdate,
    isDownloadBusy,
    loadStatus,
    checkNow,
    checkAndDownload,
    startDownload,
    startInstall,
    consumeConfirm,
    initialize,
    startPolling,
    stopPolling,
  };
});
