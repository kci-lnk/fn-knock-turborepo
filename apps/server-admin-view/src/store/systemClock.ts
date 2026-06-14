import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { browserT } from "@fn-knock/i18n/vue";
import { SystemAPI, type SystemClockStatus } from "../lib/api";

const POLL_HEALTHY_MS = 10 * 60 * 1000;
const POLL_ATTENTION_MS = 30 * 1000;

export const useSystemClockStore = defineStore("system-clock", () => {
  const status = ref<SystemClockStatus | null>(null);
  const isLoading = ref(false);
  const hasInitialized = ref(false);
  const isRefreshingLocal = ref(false);
  const isSyncingLocal = ref(false);
  let pollTimer: number | null = null;
  let pollingStarted = false;

  const shouldShowBanner = computed(
    () => status.value?.needsAttention === true,
  );
  const isRefreshing = computed(
    () => isRefreshingLocal.value || status.value?.checking === true,
  );
  const isSyncing = computed(
    () => isSyncingLocal.value || status.value?.syncInProgress === true,
  );

  const clearTimer = () => {
    if (pollTimer !== null) {
      window.clearTimeout(pollTimer);
      pollTimer = null;
    }
  };

  const schedulePoll = () => {
    clearTimer();
    if (!pollingStarted) return;

    const delay = status.value?.needsAttention
      ? POLL_ATTENTION_MS
      : POLL_HEALTHY_MS;

    pollTimer = window.setTimeout(async () => {
      const current = await loadStatus(true);
      if (!current?.checkedAt || current.lastCheckError) {
        await refresh(false);
      }
      schedulePoll();
    }, delay);
  };

  async function loadStatus(silent = false) {
    if (!silent) isLoading.value = true;
    try {
      status.value = await SystemAPI.getClockStatus();
      return status.value;
    } catch (error) {
      if (!silent) {
        toast.error(browserT("admin.systemClock.loadFailed"), {
          description: extractErrorMessage(error, browserT("common.tryLater")),
        });
      }
      return null;
    } finally {
      if (!silent) isLoading.value = false;
    }
  }

  async function refresh(showToast = true) {
    if (isRefreshingLocal.value) return false;

    isRefreshingLocal.value = true;
    try {
      status.value = await SystemAPI.refreshClockStatus();
      if (showToast) {
        if (status.value.needsAttention) {
          toast.error(browserT("admin.systemClock.stillAbnormal"));
        } else {
          toast.success(browserT("admin.systemClock.recovered"));
        }
      }
      return true;
    } catch (error) {
      if (showToast) {
        toast.error(browserT("admin.systemClock.refreshFailed"), {
          description: extractErrorMessage(error, browserT("common.tryLater")),
        });
      }
      return false;
    } finally {
      isRefreshingLocal.value = false;
      schedulePoll();
    }
  }

  async function sync() {
    if (isSyncingLocal.value) return false;

    isSyncingLocal.value = true;
    try {
      const result = await SystemAPI.syncClock();
      status.value = result.data;
      toast.success(result.message);
      if (status.value.needsAttention) {
        toast.error(browserT("admin.systemClock.stillNeedsRefresh"));
      }
      return true;
    } catch (error) {
      toast.error(browserT("admin.systemClock.syncFailed"), {
        description: extractErrorMessage(error, browserT("common.tryLater")),
      });
      return false;
    } finally {
      isSyncingLocal.value = false;
      schedulePoll();
    }
  }

  async function initialize() {
    if (hasInitialized.value) return;
    hasInitialized.value = true;

    const current = await loadStatus(true);
    if (!current?.checkedAt || current.lastCheckError) {
      await refresh(false);
    }

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
    isRefreshing,
    isSyncing,
    shouldShowBanner,
    loadStatus,
    refresh,
    sync,
    initialize,
    startPolling,
    stopPolling,
  };
});
