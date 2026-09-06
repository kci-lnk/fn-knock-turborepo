import {
  computed,
  onMounted,
  onUnmounted,
  ref,
  toValue,
  watch,
  type MaybeRefOrGetter,
} from "vue";
import { RuntimeHealthAPI } from "@/lib/api/runtime-health";
import type {
  RuntimeDebugReport,
  RuntimeDebugResponse,
} from "@/types/runtime-debug";

const POLL_INTERVAL_MS = 2_000;
type DebugAction = "start" | "stop" | "memory";

export const useRuntimeDebug = (options: {
  enabled: MaybeRefOrGetter<boolean>;
}) => {
  const report = ref<RuntimeDebugReport | null>(null);
  const loading = ref(false);
  const action = ref<DebugAction | null>(null);
  const error = ref(false);
  const unavailable = ref(false);
  let mounted = false;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let controller: AbortController | null = null;
  let generation = 0;
  let lastReadAt = -Infinity;

  const enabled = () => mounted && toValue(options.enabled) && !document.hidden;
  const clearTimer = () => {
    if (timer !== null) clearTimeout(timer);
    timer = null;
  };
  const cancelRequest = () => {
    ++generation;
    controller?.abort();
    controller = null;
    loading.value = false;
    action.value = null;
  };
  const schedule = () => {
    clearTimer();
    if (!enabled() || unavailable.value) return;
    timer = setTimeout(() => void refresh(), POLL_INTERVAL_MS);
  };
  const request = async (
    fetcher: (signal: AbortSignal) => Promise<RuntimeDebugResponse>,
    mutation: DebugAction | null = null,
  ) => {
    if (!enabled()) return;
    clearTimer();
    cancelRequest();
    const requestId = generation;
    const pendingController = new AbortController();
    controller = pendingController;
    loading.value = true;
    action.value = mutation;
    error.value = false;
    try {
      const result = await fetcher(pendingController.signal);
      if (
        requestId !== generation ||
        pendingController.signal.aborted ||
        !enabled()
      )
        return;
      report.value = result.data;
      unavailable.value = false;
    } catch (cause) {
      if (requestId !== generation || pendingController.signal.aborted) return;
      const status = (cause as { response?: { status?: number } })?.response
        ?.status;
      unavailable.value = status === 404 || status === 501;
      error.value = true;
    } finally {
      if (requestId === generation) {
        loading.value = false;
        action.value = null;
        controller = null;
        schedule();
      }
    }
  };
  const refresh = async () => {
    if (!enabled() || loading.value) return;
    const delay = POLL_INTERVAL_MS - (Date.now() - lastReadAt);
    if (delay > 0) {
      clearTimer();
      timer = setTimeout(() => void refresh(), delay);
      return;
    }
    lastReadAt = Date.now();
    await request(RuntimeHealthAPI.getDebug);
  };
  const perform = async (nextAction: DebugAction) => {
    if (action.value || !report.value || unavailable.value) return;
    const fetcher =
      nextAction === "start"
        ? RuntimeHealthAPI.startDebugCapture
        : nextAction === "stop"
          ? RuntimeHealthAPI.stopDebugCapture
          : RuntimeHealthAPI.refreshDebugMemory;
    await request(fetcher, nextAction);
  };
  const sync = () => {
    clearTimer();
    cancelRequest();
    if (enabled()) void refresh();
  };
  watch(() => toValue(options.enabled), sync);
  onMounted(() => {
    mounted = true;
    document.addEventListener("visibilitychange", sync);
    sync();
  });
  onUnmounted(() => {
    mounted = false;
    document.removeEventListener("visibilitychange", sync);
    clearTimer();
    cancelRequest();
  });

  const running = computed(() => report.value?.capture.status === "running");
  const remainingSeconds = computed(() => {
    const capture = report.value?.capture;
    return capture
      ? Math.max(
          0,
          Math.ceil(capture.duration_seconds - capture.elapsed_ms / 1000),
        )
      : 0;
  });
  return {
    report,
    loading,
    action,
    error,
    unavailable,
    running,
    remainingSeconds,
    refresh,
    start: () => perform("start"),
    stop: () => perform("stop"),
    refreshMemory: () => perform("memory"),
  };
};
