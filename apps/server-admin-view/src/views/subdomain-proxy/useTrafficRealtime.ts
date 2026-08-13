import { ref } from "vue";
import type { TrafficStats } from "@/types";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";

export const useTrafficRealtime = ({
  intervalMs = 1000,
  load,
  onError,
}: {
  intervalMs?: number;
  load: () => Promise<TrafficStats>;
  onError?: (error: unknown) => void;
}) => {
  const trafficRealtimeStats = ref<TrafficStats | null>(null);

  const loadTrafficRealtime = async () => {
    try {
      trafficRealtimeStats.value = await load();
    } catch (error) {
      onError?.(error);
    }
  };

  const poller = createVisibilityPoller({
    intervalMs,
    task: loadTrafficRealtime,
  });

  const stopTrafficRealtimePolling = () => {
    poller.stop();
  };

  const startTrafficRealtimePolling = () => {
    poller.start();
  };

  return {
    loadTrafficRealtime,
    startTrafficRealtimePolling,
    stopTrafficRealtimePolling,
    trafficRealtimeStats,
  };
};
