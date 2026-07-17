import { computed, ref, type Ref } from "vue";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "../../lib/api";
import type {
  AuthLoginMode,
  AuthLoginModePreview,
  AuthLoginModeStatus,
} from "../../types";

interface UseAuthModeSwitchOptions {
  authLoginMode: Ref<AuthLoginMode>;
  authModeStatus: Ref<AuthLoginModeStatus | null>;
  refreshStatus: () => Promise<unknown>;
  translate: (key: string) => string;
}

export function useAuthModeSwitch({
  authLoginMode,
  authModeStatus,
  refreshStatus,
  translate,
}: UseAuthModeSwitchOptions) {
  const authModePreview = ref<AuthLoginModePreview | null>(null);
  const showAuthModeSwitchDialog = ref(false);
  const targetAuthLoginMode = computed<AuthLoginMode>(() =>
    authLoginMode.value === "totp" ? "password" : "totp",
  );
  const { isPending: isPreviewingAuthMode, run: runPreviewAuthMode } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.authSettings.previewAuthModeFailed"),
          ),
        );
      },
    });
  const { isPending: isSwitchingAuthMode, run: runSwitchAuthMode } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.authSettings.switchAuthModeFailed"),
          ),
        );
      },
    });
  const isAuthModeBusy = computed(
    () => isPreviewingAuthMode.value || isSwitchingAuthMode.value,
  );

  const refreshAuthModePreview = async () => {
    await runPreviewAuthMode(async () => {
      authModePreview.value = await ConfigAPI.previewAuthLoginMode(
        targetAuthLoginMode.value,
      );
    });
  };

  const openAuthModeSwitchDialog = async () => {
    if (isAuthModeBusy.value) return;
    authModePreview.value = null;
    showAuthModeSwitchDialog.value = true;
    await refreshAuthModePreview();
  };

  const handleSwitchAuthMode = async () => {
    await runSwitchAuthMode(async () => {
      authModeStatus.value = await ConfigAPI.switchAuthLoginMode(
        targetAuthLoginMode.value,
      );
      showAuthModeSwitchDialog.value = false;
      authModePreview.value = null;
      await refreshStatus();
      toast.success(translate("admin.authSettings.switchAuthModeCompleted"));
    });
  };

  return {
    authModePreview,
    handleSwitchAuthMode,
    isAuthModeBusy,
    isPreviewingAuthMode,
    isSwitchingAuthMode,
    openAuthModeSwitchDialog,
    refreshAuthModePreview,
    showAuthModeSwitchDialog,
  };
}
