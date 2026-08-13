import { onUnmounted, ref, watch } from "vue";
import { DeepMonitorAPI } from "@/lib/api/deep-monitor";
import { normalizeHostLike } from "./model";
import { createVisibilityPoller } from "@/composables/useVisibilityPolling";

export const useActiveDeepMonitors = (enabled: () => boolean) => {
  const activeHosts = ref<string[]>([]);
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

  const poller = createVisibilityPoller({
    intervalMs: 5_000,
    enabled,
    task: refresh,
  });
  poller.start();

  watch(
    enabled,
    (available) => {
      poller.sync();
      if (!available) {
        activeHosts.value = [];
      }
    },
    { immediate: true },
  );
  onUnmounted(poller.stop);

  return activeHosts;
};
