import { defineStore } from "pinia";
import { ref } from "vue";
import {
  TerminalAccessAPI,
  type WebTerminalAccessStatus,
  type WebTerminalSettings,
} from "@/lib/api/terminal-access";

export const useTerminalAccessStore = defineStore("terminal-access", () => {
  const status = ref<WebTerminalAccessStatus | null>(null);
  let generation = 0;
  function invalidate() {
    generation++;
    if (status.value) status.value = { ...status.value, authorized: false };
  }
  function applySettings(settings: WebTerminalSettings) {
    generation++;
    status.value = {
      ...settings,
      authorized:
        settings.enabled &&
        (!settings.passwordConfigured ||
          (status.value?.revision === settings.revision &&
            status.value.authorized)),
    };
  }
  async function refresh() {
    const request = ++generation;
    try {
      const next = await TerminalAccessAPI.status();
      if (request === generation) status.value = next;
      return next;
    } catch (error) {
      // A failed newer check must not leave an older authorization usable.
      if (request === generation) invalidate();
      throw error;
    }
  }
  return { status, invalidate, applySettings, refresh };
});
