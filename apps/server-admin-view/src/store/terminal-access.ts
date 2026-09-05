import { defineStore } from "pinia";
import { ref } from "vue";
import {
  TerminalAccessAPI,
  type WebTerminalSettings,
} from "@/lib/api/terminal-access";

export const useTerminalAccessStore = defineStore("terminal-access", () => {
  const status = ref<WebTerminalSettings | null>(null);
  const isCurrent = ref(false);
  let generation = 0;
  function invalidate() {
    generation++;
    isCurrent.value = false;
  }
  function applySettings(settings: WebTerminalSettings) {
    generation++;
    status.value = settings;
    isCurrent.value = true;
  }
  async function refresh() {
    const request = ++generation;
    try {
      const next = await TerminalAccessAPI.settings();
      if (request === generation) {
        status.value = next;
        isCurrent.value = true;
      }
      return next;
    } catch (error) {
      // A failed newer check must not leave an older enabled state usable.
      if (request === generation) invalidate();
      throw error;
    }
  }
  return { status, isCurrent, invalidate, applySettings, refresh };
});
