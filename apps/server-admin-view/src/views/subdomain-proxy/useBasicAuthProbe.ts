import { computed, onUnmounted, ref, watch, type Ref } from "vue";
import { type HostMappingBasicAuthProbeResult } from "@/lib/api/config";
import { normalizeBasicAuthProbeTarget } from "./model";

export const useBasicAuthProbe = ({
  delayMs = 450,
  enabled,
  getErrorMessage,
  probe,
  target,
}: {
  delayMs?: number;
  enabled: Ref<boolean>;
  getErrorMessage: (error: unknown) => string;
  probe: (target: string) => Promise<HostMappingBasicAuthProbeResult>;
  target: Ref<string>;
}) => {
  const basicAuthProbeCache = ref(
    new Map<string, HostMappingBasicAuthProbeResult>(),
  );
  const isLoadingBasicAuthProbe = ref(false);
  const basicAuthProbeTargetKey = computed(() =>
    normalizeBasicAuthProbeTarget(target.value),
  );
  const currentBasicAuthProbeResult = computed(() => {
    const normalizedTarget = basicAuthProbeTargetKey.value;
    if (!normalizedTarget) return null;
    return basicAuthProbeCache.value.get(normalizedTarget) ?? null;
  });

  let basicAuthProbeTimer: number | null = null;
  let basicAuthProbeRequestId = 0;

  const setBasicAuthProbeCacheResult = (
    normalizedTarget: string,
    result: HostMappingBasicAuthProbeResult,
  ) => {
    const next = new Map(basicAuthProbeCache.value);
    next.set(normalizedTarget, result);
    basicAuthProbeCache.value = next;
  };

  const clearBasicAuthProbeTimer = () => {
    if (basicAuthProbeTimer === null) return;
    window.clearTimeout(basicAuthProbeTimer);
    basicAuthProbeTimer = null;
  };

  const cancelBasicAuthProbe = () => {
    clearBasicAuthProbeTimer();
    basicAuthProbeRequestId += 1;
    isLoadingBasicAuthProbe.value = false;
  };

  const runBasicAuthProbe = async (normalizedTarget: string) => {
    if (!normalizedTarget) {
      isLoadingBasicAuthProbe.value = false;
      return;
    }
    if (basicAuthProbeCache.value.has(normalizedTarget)) {
      isLoadingBasicAuthProbe.value = false;
      return;
    }

    const requestId = ++basicAuthProbeRequestId;
    isLoadingBasicAuthProbe.value = true;

    try {
      const result = await probe(normalizedTarget);
      setBasicAuthProbeCacheResult(normalizedTarget, result);
    } catch (error) {
      setBasicAuthProbeCacheResult(normalizedTarget, {
        requiresBasicAuth: false,
        httpStatus: null,
        error: getErrorMessage(error),
      });
    } finally {
      if (
        requestId === basicAuthProbeRequestId &&
        basicAuthProbeTargetKey.value === normalizedTarget
      ) {
        isLoadingBasicAuthProbe.value = false;
      }
    }
  };

  const scheduleBasicAuthProbe = () => {
    clearBasicAuthProbeTimer();

    const normalizedTarget = basicAuthProbeTargetKey.value;
    if (
      !enabled.value ||
      !normalizedTarget ||
      basicAuthProbeCache.value.has(normalizedTarget)
    ) {
      basicAuthProbeRequestId += 1;
      isLoadingBasicAuthProbe.value = false;
      return;
    }

    isLoadingBasicAuthProbe.value = true;
    basicAuthProbeTimer = window.setTimeout(() => {
      basicAuthProbeTimer = null;
      void runBasicAuthProbe(normalizedTarget);
    }, delayMs);
  };

  watch([enabled, basicAuthProbeTargetKey], () => {
    scheduleBasicAuthProbe();
  });

  onUnmounted(() => {
    cancelBasicAuthProbe();
  });

  return {
    basicAuthProbeTargetKey,
    cancelBasicAuthProbe,
    currentBasicAuthProbeResult,
    isLoadingBasicAuthProbe,
    scheduleBasicAuthProbe,
  };
};
