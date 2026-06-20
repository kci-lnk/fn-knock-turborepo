import { ref } from "vue";
import type { TrafficStats } from "@/types";

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
  let trafficRealtimeTimer: number | null = null;
  let isTrafficRealtimeLoading = false;

  const loadTrafficRealtime = async () => {
    if (isTrafficRealtimeLoading) return;
    isTrafficRealtimeLoading = true;
    try {
      trafficRealtimeStats.value = await load();
    } catch (error) {
      onError?.(error);
    } finally {
      isTrafficRealtimeLoading = false;
    }
  };

  const stopTrafficRealtimePolling = () => {
    if (trafficRealtimeTimer !== null) {
      window.clearInterval(trafficRealtimeTimer);
      trafficRealtimeTimer = null;
    }
  };

  const startTrafficRealtimePolling = () => {
    stopTrafficRealtimePolling();
    void loadTrafficRealtime();
    trafficRealtimeTimer = window.setInterval(() => {
      void loadTrafficRealtime();
    }, intervalMs);
  };

  return {
    loadTrafficRealtime,
    startTrafficRealtimePolling,
    stopTrafficRealtimePolling,
    trafficRealtimeStats,
  };
};
