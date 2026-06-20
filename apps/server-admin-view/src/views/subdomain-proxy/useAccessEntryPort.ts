import { ref } from "vue";
import { SystemAPI } from "@/lib/api";

export const useAccessEntryPort = () => {
  const accessEntryPort = ref("7999");

  const loadAccessEntryPort = async () => {
    try {
      const info = await SystemAPI.getAccessEntry();
      accessEntryPort.value = info.port.trim() || "7999";
    } catch (error) {
      console.warn("load access entry port failed:", error);
    }
  };

  return {
    accessEntryPort,
    loadAccessEntryPort,
  };
};
