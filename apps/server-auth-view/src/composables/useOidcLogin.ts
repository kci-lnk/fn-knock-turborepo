import { ref, type Ref } from "vue";
import { apiClient } from "@/lib/api";

interface UseOidcLoginOptions {
  clearError: () => void;
  isLoginCoolingDown: Ref<boolean>;
  redirectUri: string | null;
  rememberMe: Ref<boolean>;
  reportError: (message: string) => void;
  resetRedirectGuard: () => void;
  translate: (key: string) => string;
}

export function useOidcLogin({
  clearError,
  isLoginCoolingDown,
  redirectUri,
  rememberMe,
  reportError,
  resetRedirectGuard,
  translate,
}: UseOidcLoginOptions) {
  const isOidcLoading = ref(false);
  const activeOidcProviderId = ref("");

  const handleOidcLogin = async (providerId: string) => {
    if (isOidcLoading.value || isLoginCoolingDown.value) return;
    isOidcLoading.value = true;
    activeOidcProviderId.value = providerId;
    clearError();
    try {
      const response = await apiClient.post("/oidc/start", {
        provider_id: providerId,
        mode: "login",
        rememberMe: rememberMe.value,
        redirect_uri: redirectUri || undefined,
      });
      const authorizationUrl = response.data?.data?.authorization_url;
      if (!authorizationUrl) {
        throw new Error(
          response.data?.message || translate("auth.oidcStartFailed"),
        );
      }
      resetRedirectGuard();
      window.location.assign(authorizationUrl);
    } catch (error: any) {
      reportError(
        error?.response?.data?.message ||
          error?.message ||
          translate("auth.oidcLoginFailed"),
      );
      isOidcLoading.value = false;
      activeOidcProviderId.value = "";
    }
  };

  return {
    activeOidcProviderId,
    handleOidcLogin,
    isOidcLoading,
  };
}
