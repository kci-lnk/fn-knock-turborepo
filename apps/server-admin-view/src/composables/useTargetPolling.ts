import { ref } from 'vue';
import { PollingAPI, type PollTarget, type PollingPayloadMap } from '../lib/api';

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
  let timer: number | null = null;
  let cursor: number | undefined;
  let inFlight = false;
  let runToken = 0;

  const clearTimer = () => {
    if (timer !== null) {
      window.clearInterval(timer);
      timer = null;
    }
  };

  const resetCursor = () => {
    cursor = undefined;
  };

  const refresh = async () => {
    if (inFlight) return;
    const token = runToken;
    inFlight = true;
    try {
      const payload = await PollingAPI.poll(options.target, cursor);
      if (token !== runToken) return;
      const nextCursor = (payload as { cursor?: unknown }).cursor;
      if (typeof nextCursor === 'number' && Number.isFinite(nextCursor) && nextCursor >= 0) {
        cursor = nextCursor;
      }
      options.onData(payload);
    } catch (error) {
      options.onError?.(error);
    } finally {
      inFlight = false;
    }
  };

  const start = () => {
    if (timer !== null) return;
    runToken += 1;
    isRunning.value = true;
    if (options.immediate !== false) {
      void refresh();
    }
    timer = window.setInterval(() => {
      void refresh();
    }, options.intervalMs ?? 2000);
  };

  const stop = () => {
    runToken += 1;
    isRunning.value = false;
    clearTimer();
  };

  return {
    isRunning,
    start,
    stop,
    refresh,
    resetCursor,
  };
}
