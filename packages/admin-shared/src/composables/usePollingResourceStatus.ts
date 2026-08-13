import { onMounted, onUnmounted, ref } from "vue";
import { createVisibilityPoller } from "./createVisibilityPoller";

interface UsePollingResourceStatusOptions<T> {
  fetcher: (signal?: AbortSignal) => Promise<T>;
  onData: (data: T) => void;
  isDownloading: (data: T) => boolean;
  onError?: (error: unknown) => void;
  intervalMs?: number;
}

export function usePollingResourceStatus<T>(
  options: UsePollingResourceStatusOptions<T>,
) {
  const isInitializing = ref(true);
  let shouldKeepPolling = false;
  const poller = createVisibilityPoller({
    intervalMs: options.intervalMs ?? 1000,
    immediate: false,
    task: async (signal) => {
      try {
        const data = await options.fetcher(signal);
        if (signal.aborted) return;
        options.onData(data);
        shouldKeepPolling = options.isDownloading(data);
        if (!shouldKeepPolling) poller.stop();
      } catch (error) {
        // Visibility changes and component teardown deliberately abort a
        // request. Do not interpret that as a completed resource and stop the
        // poller permanently; it must remain able to resume when visible.
        if (signal.aborted) return;
        options.onError?.(error);
        if (!shouldKeepPolling) poller.stop();
      } finally {
        isInitializing.value = false;
      }
    },
  });

  const refresh = () => {
    // A completed resource stops periodic polling. Explicit refreshes after a
    // user action restart it and immediately execute one non-overlapping pass.
    poller.start();
    return poller.refresh();
  };

  const stopPolling = () => poller.stop();

  onMounted(() => void refresh());
  onUnmounted(stopPolling);

  return {
    isInitializing,
    refresh,
    stopPolling,
  };
}
