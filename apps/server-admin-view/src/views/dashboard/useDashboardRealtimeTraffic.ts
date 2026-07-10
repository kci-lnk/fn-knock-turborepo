import { ref } from "vue";
import type { TrafficStats } from "@/types";
import { useTargetPolling } from "@/composables/useTargetPolling";

export const useDashboardRealtimeTraffic = () => {
  const realtimeStats = ref<TrafficStats | null>(null);
  const realtimeInBps = ref<number | null>(null);
  const realtimeOutBps = ref<number | null>(null);
  let previousSample: {
    at: number;
    totalIn: number;
    totalOut: number;
  } | null = null;

  const apply = (payload: TrafficStats) => {
    if (
      !Number.isFinite(payload.total_in) ||
      !Number.isFinite(payload.total_out)
    ) {
      return;
    }

    realtimeStats.value = payload;
    const timestamp = Number(payload.timestamp ?? Date.now());
    if (previousSample) {
      const elapsedSeconds = Math.max(
        1,
        (timestamp - previousSample.at) / 1000,
      );
      realtimeInBps.value =
        Math.max(0, payload.total_in - previousSample.totalIn) / elapsedSeconds;
      realtimeOutBps.value =
        Math.max(0, payload.total_out - previousSample.totalOut) /
        elapsedSeconds;
    } else {
      realtimeInBps.value = null;
      realtimeOutBps.value = null;
    }

    previousSample = {
      at: timestamp,
      totalIn: payload.total_in,
      totalOut: payload.total_out,
    };
  };

  const polling = useTargetPolling({
    target: "dashboard",
    intervalMs: 1000,
    onData: apply,
  });

  return {
    polling,
    realtimeInBps,
    realtimeOutBps,
    realtimeStats,
  };
};
