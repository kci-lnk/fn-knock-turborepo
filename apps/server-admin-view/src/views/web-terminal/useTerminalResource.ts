import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  shallowRef,
  watch,
  type Ref,
} from "vue";
import { type TerminalAttachmentRecord } from "@/lib/api/terminal";

const REFRESH_MS = 5_000;
const RETRY_MS = 15_000;
const STALE_MS = 15_000;

/** Owns only metrics polling. Never changes the terminal attachment or its role. */
export type TerminalResourceContext = {
  attachment: Readonly<Ref<TerminalAttachmentRecord | null>>;
  sessionId: Readonly<Ref<string | null>>;
  connected: Readonly<Ref<boolean>>;
};
type ResourceSample = {
  sampledAt: string;
  sampleAgeMs: number;
  status: string;
};
export const useTerminalResource = <T extends ResourceSample>(
  { attachment, sessionId, connected }: TerminalResourceContext,
  load: (id: string, signal: AbortSignal) => Promise<T>,
) => {
  const metrics = shallowRef<T | null>(null);
  const loading = ref(false);
  const failed = ref(false);
  const now = ref(performance.now());
  const sampleStartedAt = ref<number | null>(null);
  const visible = ref(false);
  let mounted = false;
  let generation = 0;
  let cancelRequest: (() => void) | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let clock: ReturnType<typeof setInterval> | undefined;
  const stale = computed(
    () =>
      sampleStartedAt.value !== null &&
      now.value - sampleStartedAt.value > STALE_MS,
  );
  const activeId = computed(() =>
    visible.value &&
    connected.value &&
    attachment.value?.sessionId === sessionId.value
      ? (attachment.value?.id ?? null)
      : null,
  );

  const cancel = () => {
    generation += 1;
    cancelRequest?.();
    cancelRequest = null;
    clearTimeout(timer);
    loading.value = false;
  };
  const poll = async (id: string, operation: number) => {
    if (!mounted || operation !== generation) return;
    const controller = new AbortController();
    loading.value = !metrics.value;
    let delay = REFRESH_MS;
    // Bound even a lost HTTP request; backend sampling itself is limited to 4s.
    const timeout = setTimeout(() => controller.abort(), 8_000);
    cancelRequest = () => {
      clearTimeout(timeout);
      controller.abort();
    };
    try {
      const data = await load(id, controller.signal);
      if (operation !== generation || controller.signal.aborted) return;
      failed.value = data.status === "unavailable";
      // Keep the last useful sample on a transport/whole-collector failure;
      // preserve its age anchor so the UI can explicitly show when it is stale.
      if (
        !failed.value ||
        !metrics.value ||
        metrics.value.status === "unavailable"
      ) {
        // Use server-measured age plus browser monotonic elapsed time. Neither
        // clock skew nor NTP/user clock adjustments can revive an old sample.
        if (
          metrics.value?.sampledAt !== data.sampledAt ||
          data.sampleAgeMs != null
        ) {
          sampleStartedAt.value =
            performance.now() - Math.max(0, data.sampleAgeMs ?? 0);
        }
        metrics.value = data;
      }
      if (failed.value) delay = RETRY_MS;
    } catch {
      if (operation !== generation) return;
      failed.value = true;
      delay = RETRY_MS;
    } finally {
      clearTimeout(timeout);
      if (operation === generation) {
        now.value = performance.now();
        loading.value = false;
        cancelRequest = null;
        timer = setTimeout(() => void poll(id, operation), delay);
      }
    }
  };
  const restart = () => {
    cancel();
    if (mounted && activeId.value) void poll(activeId.value, generation);
  };
  // Clear synchronously before an old request can publish to a newly selected tab.
  watch(
    [sessionId, () => attachment.value?.id, activeId],
    ([nextSession, nextAttachment], [oldSession, oldAttachment]) => {
      if (nextSession !== oldSession || nextAttachment !== oldAttachment) {
        metrics.value = null;
        sampleStartedAt.value = null;
        failed.value = false;
      }
      restart();
    },
    { flush: "sync" },
  );
  const visibilityChanged = () => {
    now.value = performance.now();
    visible.value = document.visibilityState !== "hidden";
  };
  onMounted(() => {
    mounted = true;
    visibilityChanged();
    document.addEventListener("visibilitychange", visibilityChanged);
    clock = setInterval(() => {
      now.value = performance.now();
    }, 1_000);
  });
  onBeforeUnmount(() => {
    mounted = false;
    cancel();
    clearInterval(clock);
    document.removeEventListener("visibilitychange", visibilityChanged);
  });
  return {
    metrics,
    metricsLoading: loading,
    metricsFailed: failed,
    metricsStale: stale,
  };
};
