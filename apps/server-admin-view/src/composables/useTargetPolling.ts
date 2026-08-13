import { ref } from "vue";
import {
  PollingAPI,
  type PollTarget,
  type PollingPayloadMap,
} from "@/lib/api/polling";
import { createVisibilityPoller } from "./useVisibilityPolling";

interface UseTargetPollingOptions<T extends PollTarget> {
  target: T;
  intervalMs?: number;
  immediate?: boolean;
  onData: (payload: PollingPayloadMap[T]) => void;
  onError?: (error: unknown) => void;
}

export function useTargetPolling<T extends PollTarget>(
  options: UseTargetPollingOptions<T>,
) {
  const isRunning = ref(false);
  let cursor: number | undefined;
  let runToken = 0;

  const resetCursor = () => {
    cursor = undefined;
  };

  const fetchOnce = async (signal: AbortSignal) => {
    const token = runToken;
    try {
      const payload = await PollingAPI.poll(options.target, cursor, signal);
      if (token !== runToken || signal.aborted) return;
      const nextCursor = (payload as { cursor?: unknown }).cursor;
      if (
        typeof nextCursor === "number" &&
        Number.isFinite(nextCursor) &&
        nextCursor >= 0
      ) {
        cursor = nextCursor;
      }
      options.onData(payload);
    } catch (error) {
      if (!signal.aborted) options.onError?.(error);
    }
  };

  const poller = createVisibilityPoller({
    intervalMs: options.intervalMs ?? 2000,
    immediate: options.immediate,
    task: fetchOnce,
  });

  const start = () => {
    if (isRunning.value) return;
    runToken += 1;
    isRunning.value = true;
    poller.start();
  };

  const stop = () => {
    runToken += 1;
    isRunning.value = false;
    poller.stop();
  };

  return {
    isRunning,
    start,
    stop,
    refresh: poller.refresh,
    resetCursor,
  };
}
