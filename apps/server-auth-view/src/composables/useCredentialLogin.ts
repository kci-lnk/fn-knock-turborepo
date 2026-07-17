import { computed, ref, type Ref } from "vue";
import type { CaptchaSubmission } from "@frontend-core/captcha/types";
import { apiClient } from "@/lib/api";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

interface UseCredentialLoginOptions {
  captchaSubmission: Ref<CaptchaSubmission | null>;
  clearError: () => void;
  handleLoginSuccess: (payload: {
    passkey: any;
    redirectTo: string | null;
    runType: 0 | 1 | 3;
  }) => Promise<unknown>;
  isCaptchaVerified: Ref<boolean>;
  isLoginCompletionPending: () => boolean;
  isLoginCoolingDown: Ref<boolean>;
  isPasskeySupported: Ref<boolean>;
  loginCooldownSeconds: Ref<number>;
  loginMode: Ref<"totp" | "password">;
  password: Ref<string>;
  redirectUri: string | null;
  rememberMe: Ref<boolean>;
  reportError: (message: string) => void;
  resetCaptchaWidgets: () => void;
  resolveLoginCooldownMessage: (message: string, source?: unknown) => string;
  token: Ref<string>;
  translate: Translate;
  username: Ref<string>;
}

export function useCredentialLogin({
  captchaSubmission,
  clearError,
  handleLoginSuccess,
  isCaptchaVerified,
  isLoginCompletionPending,
  isLoginCoolingDown,
  isPasskeySupported,
  loginCooldownSeconds,
  loginMode,
  password,
  redirectUri,
  rememberMe,
  reportError,
  resetCaptchaWidgets,
  resolveLoginCooldownMessage,
  token,
  translate,
  username,
}: UseCredentialLoginOptions) {
  const isLoading = ref(false);
  let lastLoginAttemptAt = 0;
  const loginButtonLabel = computed(() => {
    if (isLoading.value) return translate("auth.verifying");
    if (isLoginCoolingDown.value) {
      return translate("auth.retryAfterSeconds", {
        seconds: loginCooldownSeconds.value,
      });
    }
    return translate("auth.verifyNow");
  });

  const resetLoginState = () => {
    token.value = "";
    password.value = "";
    resetCaptchaWidgets();
  };

  const handleLogin = async () => {
    if (
      isLoading.value ||
      isLoginCoolingDown.value ||
      isLoginCompletionPending()
    ) {
      return;
    }
    if (loginMode.value === "totp" && token.value.length !== 6) {
      reportError(translate("auth.invalidOtpLength"));
      return;
    }
    if (
      loginMode.value === "password" &&
      (!username.value.trim() || !password.value)
    ) {
      reportError(translate("auth.usernamePasswordRequired"));
      return;
    }
    if (!isCaptchaVerified.value || !captchaSubmission.value) {
      reportError(translate("auth.captchaFirst"));
      return;
    }

    const now = Date.now();
    if (now - lastLoginAttemptAt < 400) return;
    lastLoginAttemptAt = now;
    isLoading.value = true;
    clearError();

    try {
      const response = await apiClient.post("/login", {
        method: loginMode.value,
        token: loginMode.value === "totp" ? token.value : undefined,
        username:
          loginMode.value === "password" ? username.value.trim() : undefined,
        password: loginMode.value === "password" ? password.value : undefined,
        captcha: captchaSubmission.value,
        rememberMe: rememberMe.value,
        redirect_uri: redirectUri || undefined,
      });

      if (response.data.success) {
        const runType = (response.data.data?.run_type ?? 3) as 0 | 1 | 3;
        const redirectTo =
          typeof response.data.data?.redirect_to === "string"
            ? response.data.data.redirect_to
            : null;
        const passkey = isPasskeySupported.value
          ? response.data.data?.passkey
          : null;
        await handleLoginSuccess({ passkey, redirectTo, runType });
      } else {
        reportError(
          resolveLoginCooldownMessage(
            response.data.message || translate("auth.loginFailed"),
            response.data,
          ),
        );
        resetLoginState();
      }
    } catch (error: any) {
      reportError(
        resolveLoginCooldownMessage(
          error?.response?.data?.message || translate("auth.loginFailed"),
          error,
        ),
      );
      resetLoginState();
    } finally {
      isLoading.value = false;
    }
  };

  const handleOtpComplete = () => void handleLogin();

  return {
    handleLogin,
    handleOtpComplete,
    isLoading,
    loginButtonLabel,
    resetLoginState,
  };
}
