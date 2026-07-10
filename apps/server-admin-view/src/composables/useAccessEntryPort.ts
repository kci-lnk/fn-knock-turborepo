import { ref } from "vue";
import { SystemAPI } from "@/lib/api";

interface UseAccessEntryPortOptions {
  fallbackPort?: string;
  onError?: (error: unknown) => void;
}

export function useAccessEntryPort({
  fallbackPort = "7999",
  onError = (error) => {
    console.warn("load access entry port failed:", error);
  },
}: UseAccessEntryPortOptions = {}) {
  const accessEntryPort = ref(fallbackPort);

  async function loadAccessEntryPort() {
    try {
      const info = await SystemAPI.getAccessEntry();
      accessEntryPort.value = info.port.trim() || fallbackPort;
    } catch (error) {
      onError(error);
    }
  }

  return {
    accessEntryPort,
    loadAccessEntryPort,
  };
}
