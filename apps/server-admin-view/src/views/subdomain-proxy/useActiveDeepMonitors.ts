import { onUnmounted, ref, watch } from "vue";
import { DeepMonitorAPI } from "@/lib/api";
import { normalizeHostLike } from "./model";

export const useActiveDeepMonitors = (enabled: () => boolean) => {
  const activeHosts = ref<string[]>([]);
  let refreshTimer: number | undefined;

  const stop = () => {
    if (refreshTimer) window.clearInterval(refreshTimer);
    refreshTimer = undefined;
  };

  const refresh = async () => {
    try {
      const sessions = await DeepMonitorAPI.list();
      activeHosts.value = sessions
        .filter((session) => session.state === "active")
        .map((session) => normalizeHostLike(session.host));
    } catch (error) {
      console.warn("load active deep monitors failed:", error);
    }
  };

  const start = () => {
    stop();
    void refresh();
    refreshTimer = window.setInterval(() => void refresh(), 5000);
  };

  watch(
    enabled,
    (available) => {
      if (available) start();
      else {
        stop();
        activeHosts.value = [];
      }
    },
    { immediate: true },
  );
  onUnmounted(stop);

  return activeHosts;
};
