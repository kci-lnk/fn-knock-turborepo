import { computed, getCurrentScope, onScopeDispose, ref } from "vue";

interface IntervalScheduler {
  setInterval(callback: () => void, intervalMs: number): unknown;
  clearInterval(handle: unknown): void;
}

interface UseLoginCooldownOptions {
  formatRetrySuffix: (seconds: number) => string;
  scheduler?: IntervalScheduler | null;
  tickMs?: number;
}

const getDefaultScheduler = (): IntervalScheduler | null => {
  if (typeof window === "undefined") {
    return null;
  }

  return {
    setInterval: (callback, intervalMs) =>
      window.setInterval(callback, intervalMs),
    clearInterval: (handle) => window.clearInterval(handle as number),
  };
};

const readRetryAfterValue = (payload: unknown): unknown => {
  if (!payload || typeof payload !== "object") {
    return undefined;
  }

  const candidate = payload as {
    retryAfter?: unknown;
    response?: {
      data?: { retryAfter?: unknown };
      headers?: Record<string, unknown>;
    };
  };

  return (
    candidate.retryAfter ??
    candidate.response?.data?.retryAfter ??
    candidate.response?.headers?.["retry-after"]
  );
};

export const extractRetryAfterSeconds = (payload: unknown): number => {
  const rawRetryAfter = readRetryAfterValue(payload);
  const retryAfterValue = Array.isArray(rawRetryAfter)
    ? rawRetryAfter[0]
    : rawRetryAfter;
  const parsedSeconds = Number(retryAfterValue);

  return Number.isFinite(parsedSeconds) && parsedSeconds > 0
    ? Math.ceil(parsedSeconds)
    : 0;
};

export const appendRetryAfterSuffix = (
  message: string,
  retryAfter: number,
  retrySuffix: string,
) => {
  if (
    retryAfter <= 0 ||
    message.includes(String(retryAfter)) ||
    message.includes(retrySuffix)
  ) {
    return message;
  }

  return `${message}${retrySuffix}`;
};

export const useLoginCooldown = (options: UseLoginCooldownOptions) => {
  const remainingSeconds = ref(0);
  const isCoolingDown = computed(() => remainingSeconds.value > 0);
  const scheduler =
    options.scheduler === undefined ? getDefaultScheduler() : options.scheduler;
  let timer: unknown = null;

  const stop = () => {
    if (timer === null) {
      return;
    }

    scheduler?.clearInterval(timer);
    timer = null;
  };

  const start = (seconds: unknown) => {
    const parsedSeconds = Math.max(0, Math.ceil(Number(seconds) || 0));
    if (parsedSeconds <= 0) {
      return parsedSeconds;
    }

    stop();
    remainingSeconds.value = parsedSeconds;

    if (!scheduler) {
      return parsedSeconds;
    }

    timer = scheduler.setInterval(() => {
      if (remainingSeconds.value <= 1) {
        remainingSeconds.value = 0;
        stop();
        return;
      }
      remainingSeconds.value -= 1;
    }, options.tickMs ?? 1000);

    return parsedSeconds;
  };

  const resolveMessage = (message: string, payload: unknown) => {
    const retryAfter = start(extractRetryAfterSeconds(payload));
    return appendRetryAfterSuffix(
      message,
      retryAfter,
      options.formatRetrySuffix(retryAfter),
    );
  };

  if (getCurrentScope()) {
    onScopeDispose(stop);
  }

  return {
    isCoolingDown,
    remainingSeconds,
    resolveMessage,
    start,
    stop,
  };
};
