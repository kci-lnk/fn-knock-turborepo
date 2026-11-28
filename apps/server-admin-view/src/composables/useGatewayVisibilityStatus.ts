import { onUnmounted, ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "@/lib/api/config";

export const useGatewayVisibilityStatus = (fallback?: () => string) => {
  const globalVisibilityEnabled = ref(false);
  const isGlobalVisibilityLoading = ref(false);
  const globalVisibilityLoadError = ref("");
  let requestId = 0;

  const resetGlobalVisibilityStatus = () => {
    requestId += 1;
    globalVisibilityEnabled.value = false;
    isGlobalVisibilityLoading.value = false;
    globalVisibilityLoadError.value = "";
  };

  const loadGlobalVisibilityStatus = async () => {
    resetGlobalVisibilityStatus();
    const currentRequestId = requestId;
    isGlobalVisibilityLoading.value = true;
    try {
      const details = await ConfigAPI.getGatewayVisibility();
      if (currentRequestId === requestId) {
        globalVisibilityEnabled.value = details.config.enabled;
      }
    } catch (error) {
      if (currentRequestId === requestId) {
        globalVisibilityLoadError.value = extractErrorMessage(
          error,
          fallback?.() ?? "Failed to load gateway visibility",
        );
        if (!fallback) {
          console.warn("load gateway visibility status failed:", error);
        }
      }
    } finally {
      if (currentRequestId === requestId) {
        isGlobalVisibilityLoading.value = false;
      }
    }
  };

  onUnmounted(resetGlobalVisibilityStatus);

  return {
    globalVisibilityEnabled,
    isGlobalVisibilityLoading,
    globalVisibilityLoadError,
    loadGlobalVisibilityStatus,
    resetGlobalVisibilityStatus,
  };
};
