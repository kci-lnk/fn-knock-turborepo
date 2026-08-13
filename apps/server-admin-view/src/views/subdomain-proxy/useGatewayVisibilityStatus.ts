import { onUnmounted, ref } from "vue";
import { ConfigAPI } from "@/lib/api/config";

export const useGatewayVisibilityStatus = () => {
  const globalVisibilityEnabled = ref(false);
  let requestId = 0;

  const loadGlobalVisibilityStatus = async () => {
    const currentRequestId = ++requestId;
    globalVisibilityEnabled.value = false;
    try {
      const details = await ConfigAPI.getGatewayVisibility();
      if (currentRequestId === requestId) {
        globalVisibilityEnabled.value = details.config.enabled;
      }
    } catch (error) {
      if (currentRequestId === requestId) {
        globalVisibilityEnabled.value = false;
        console.warn("load gateway visibility status failed:", error);
      }
    }
  };

  onUnmounted(() => {
    requestId += 1;
  });

  return { globalVisibilityEnabled, loadGlobalVisibilityStatus };
};
