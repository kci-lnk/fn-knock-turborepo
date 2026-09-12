import { defineStore } from "pinia";
import { ref } from "vue";
import { EventCenterAPI } from "@/lib/api/events";

export const useEventAlertsStore = defineStore("event-alerts", () => {
  const hasCriticalEvents = ref(false);
  let requestId = 0;

  async function refresh(signal?: AbortSignal) {
    const id = ++requestId;
    try {
      const result = await EventCenterAPI.getEvents(
        {
          page: 1,
          limit: "1",
          search: "",
          level: "CRITICAL",
        },
        signal,
      );
      if (signal?.aborted || id !== requestId) return;
      if (result.success && result.data) {
        hasCriticalEvents.value = result.data.total > 0;
      }
    } catch {
      // Keep the last known status on transient failures.
    }
  }

  return { hasCriticalEvents, refresh };
});
