import { onMounted, onUnmounted, ref } from 'vue';

interface UsePollingResourceStatusOptions<T> {
  fetcher: () => Promise<T>;
  onData: (data: T) => void;
  isDownloading: (data: T) => boolean;
  onError?: (error: unknown) => void;
  intervalMs?: number;
}

export function usePollingResourceStatus<T>(
  options: UsePollingResourceStatusOptions<T>,
) {
  const isInitializing = ref(true);
  let pollTimer: number | null = null;

  const stopPolling = () => {
    if (pollTimer !== null) {
      window.clearInterval(pollTimer);
      pollTimer = null;
    }
  };

  const startPolling = () => {
    if (pollTimer !== null) return;
    pollTimer = window.setInterval(refresh, options.intervalMs ?? 1000);
  };

  async function refresh() {
    try {
      const data = await options.fetcher();
      options.onData(data);
      if (options.isDownloading(data)) {
        startPolling();
      } else {
        stopPolling();
      }
    } catch (error) {
      options.onError?.(error);
    } finally {
      isInitializing.value = false;
    }
  }

  onMounted(refresh);
  onUnmounted(stopPolling);

  return {
    isInitializing,
    refresh,
    stopPolling,
  };
}
