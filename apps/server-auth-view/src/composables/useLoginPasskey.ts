import { ref, type Ref } from "vue";
import {
  normalizeRequestOptions,
  serializeCredential,
} from "@frontend-core/passkey/utils";
import { apiClient } from "@/lib/api";
import { useKnownPasskeyCredentials } from "./useKnownPasskeyCredentials";
import { usePasskeyRegistration } from "./usePasskeyRegistration";

const PASSKEY_BIND_PROMPT_STORAGE_KEY =
  "server-auth-view:passkey-bind-prompt-dismissed";

type PasskeyBindOffer = {
  bind_token?: string;
  can_bind?: boolean;
  credential_ids?: unknown;
} | null;

export const useLoginPasskey = ({
  clearError,
  completeLogin,
  isLoginCoolingDown,
  isPasskeyAvailable,
  isPasskeySupported,
  redirectUri,
  rememberMe,
  reportError,
  resolveLoginCooldownMessage,
  translate,
}: {
  clearError: () => void;
  completeLogin: (runType: 0 | 1 | 3, redirectTo?: string | null) => void;
  isLoginCoolingDown: Readonly<Ref<boolean>>;
  isPasskeyAvailable: Ref<boolean>;
  isPasskeySupported: Readonly<Ref<boolean>>;
  redirectUri: string | null;
  rememberMe: Ref<boolean>;
  reportError: (message: string) => void;
  resolveLoginCooldownMessage: (message: string, payload: unknown) => string;
  translate: (key: string) => string;
}) => {
  const isPasskeyLoading = ref(false);
  const showPasskeyBindDialog = ref(false);
  const isBindingPasskey = ref(false);
  const passkeyBindError = ref("");
  const passkeyBindToken = ref("");
  const skipPasskeyBindPrompt = ref(false);
  const pendingRunType = ref<0 | 1 | 3 | null>(null);
  const pendingRedirectTo = ref<string | null>(null);
  const { registerPasskeyCredential } = usePasskeyRegistration();
  const { hasKnownPasskeyCredential, rememberKnownPasskeyCredentialId } =
    useKnownPasskeyCredentials();

  const isPasskeyBindPromptDismissed = () => {
    if (typeof window === "undefined") return false;
    try {
      return (
        window.localStorage.getItem(PASSKEY_BIND_PROMPT_STORAGE_KEY) === "1"
      );
    } catch {
      return false;
    }
  };

  const persistPasskeyBindPromptPreference = () => {
    if (typeof window === "undefined") return;
    try {
      if (skipPasskeyBindPrompt.value) {
        window.localStorage.setItem(PASSKEY_BIND_PROMPT_STORAGE_KEY, "1");
      } else {
        window.localStorage.removeItem(PASSKEY_BIND_PROMPT_STORAGE_KEY);
      }
    } catch {
      // Storage is optional; login continuation must not depend on it.
    }
  };

  const clearPendingLogin = () => {
    pendingRunType.value = null;
    pendingRedirectTo.value = null;
  };

  const finishPendingLogin = () => {
    if (pendingRunType.value === null) return;
    const runType = pendingRunType.value;
    const redirectTo = pendingRedirectTo.value;
    clearPendingLogin();
    completeLogin(runType, redirectTo);
  };

  const handleLoginSuccess = async ({
    passkey,
    redirectTo,
    runType,
  }: {
    passkey: PasskeyBindOffer;
    redirectTo: string | null;
    runType: 0 | 1 | 3;
  }) => {
    if (isPasskeySupported.value && passkey?.can_bind && passkey.bind_token) {
      if (await hasKnownPasskeyCredential(passkey.credential_ids)) {
        completeLogin(runType, redirectTo);
        return;
      }
      if (isPasskeyBindPromptDismissed()) {
        completeLogin(runType, redirectTo);
        return;
      }

      passkeyBindToken.value = passkey.bind_token;
      pendingRunType.value = runType;
      pendingRedirectTo.value = redirectTo;
      skipPasskeyBindPrompt.value = false;
      showPasskeyBindDialog.value = true;
      return;
    }
    completeLogin(runType, redirectTo);
  };

  const handlePasskeyLogin = async () => {
    if (
      !isPasskeySupported.value ||
      !isPasskeyAvailable.value ||
      isLoginCoolingDown.value ||
      isPasskeyLoading.value
    ) {
      return;
    }
    isPasskeyLoading.value = true;
    clearError();
    try {
      const optionsRes = await apiClient.post("/passkey/auth/options");
      const requestOptions = normalizeRequestOptions(optionsRes.data.data);
      const credential = await navigator.credentials.get({
        publicKey: requestOptions,
      });
      if (!credential) {
        throw new Error(translate("auth.passkeyNoResponse"));
      }
      const payload = serializeCredential(credential as PublicKeyCredential);
      const verifyRes = await apiClient.post("/passkey/auth/verify", {
        credential: payload,
        rememberMe: rememberMe.value,
        redirect_uri: redirectUri || undefined,
      });
      if (verifyRes.data.success) {
        await rememberKnownPasskeyCredentialId(payload.id);
        completeLogin(
          (verifyRes.data.data?.run_type ?? 3) as 0 | 1 | 3,
          typeof verifyRes.data.data?.redirect_to === "string"
            ? verifyRes.data.data.redirect_to
            : null,
        );
        return;
      }
      throw new Error(
        resolveLoginCooldownMessage(
          verifyRes.data.message || translate("auth.passkeyVerifyFailed"),
          verifyRes.data,
        ),
      );
    } catch (error: any) {
      reportError(
        resolveLoginCooldownMessage(
          error?.response?.data?.message ||
            error?.message ||
            translate("auth.passkeyLoginFailed"),
          error,
        ),
      );
    } finally {
      isPasskeyLoading.value = false;
    }
  };

  const handlePasskeyBind = async () => {
    if (isBindingPasskey.value) return;
    if (!passkeyBindToken.value) {
      passkeyBindError.value = translate("auth.passkeyBindInvalid");
      return;
    }
    isBindingPasskey.value = true;
    passkeyBindError.value = "";
    try {
      const { credentialId } = await registerPasskeyCredential(
        passkeyBindToken.value,
        {
          alreadyRegistered: translate("auth.passkeyAlreadyRegistered"),
          bindFailed: translate("auth.passkeyBindFailed"),
          cancelled: translate("auth.passkeyCreateCancelled"),
          noResponse: translate("auth.passkeyNoResponse"),
          unavailable: translate("auth.passkeyCreateUnavailable"),
        },
      );
      await rememberKnownPasskeyCredentialId(credentialId);
      isPasskeyAvailable.value = true;
      showPasskeyBindDialog.value = false;
      passkeyBindToken.value = "";
      skipPasskeyBindPrompt.value = false;
      finishPendingLogin();
    } catch (error: any) {
      passkeyBindError.value =
        error?.response?.data?.message ||
        error?.message ||
        translate("auth.passkeyBindFailed");
    } finally {
      isBindingPasskey.value = false;
    }
  };

  const skipPasskeyBind = () => {
    persistPasskeyBindPromptPreference();
    showPasskeyBindDialog.value = false;
    passkeyBindToken.value = "";
    passkeyBindError.value = "";
    skipPasskeyBindPrompt.value = false;
    finishPendingLogin();
  };

  const handlePasskeyBindDialogOpenChange = (open: boolean) => {
    if (open) {
      showPasskeyBindDialog.value = true;
    } else if (showPasskeyBindDialog.value) {
      skipPasskeyBind();
    }
  };

  const isLoginCompletionPending = () =>
    showPasskeyBindDialog.value ||
    pendingRunType.value !== null ||
    isBindingPasskey.value;

  return {
    handleLoginSuccess,
    handlePasskeyBind,
    handlePasskeyBindDialogOpenChange,
    handlePasskeyLogin,
    isBindingPasskey,
    isLoginCompletionPending,
    isPasskeyLoading,
    passkeyBindError,
    showPasskeyBindDialog,
    skipPasskeyBind,
    skipPasskeyBindPrompt,
  };
};
