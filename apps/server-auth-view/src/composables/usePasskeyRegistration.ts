import {
  normalizeCreationOptions,
  serializeCredential,
} from "@frontend-core/passkey/utils";
import { apiClient } from "@/lib/api";
import {
  getPasskeyErrorDetails,
  resolvePasskeyRegistrationError,
  shouldRetryPasskeyRegistrationWithStandardProfile,
} from "@/lib/passkey-errors";

type RegisterPasskeyMessages = {
  alreadyRegistered: string;
  bindFailed: string;
  cancelled: string;
  noResponse: string;
  unavailable: string;
};

const resolvePasskeyDeviceName = () =>
  (navigator as Navigator & { userAgentData?: { platform?: string } })
    .userAgentData?.platform ||
  navigator.platform ||
  "Unknown Device";

const isAndroidPasskeyClient = () => {
  const navigatorWithHints = navigator as Navigator & {
    userAgentData?: { platform?: string };
  };
  return (
    navigatorWithHints.userAgentData?.platform?.toLowerCase() === "android" ||
    navigator.userAgent.toLowerCase().includes("android")
  );
};

const passkeyCreationError = (
  error: unknown,
  creationOptions: PublicKeyCredentialCreationOptions,
  messages: RegisterPasskeyMessages,
) => {
  const details = getPasskeyErrorDetails(error);
  console.warn("Passkey credential creation failed", {
    name: details.name,
    message: details.message,
    origin: window.location.origin,
    rpId: creationOptions.rp?.id,
    authenticatorSelection: creationOptions.authenticatorSelection,
    platform: resolvePasskeyDeviceName(),
    secureContext: window.isSecureContext,
  });
  const registrationError = new Error(
    resolvePasskeyRegistrationError(error, {
      alreadyRegistered: messages.alreadyRegistered,
      cancelled: messages.cancelled,
      failed: messages.bindFailed,
      unavailable: messages.unavailable,
    }),
  );
  registrationError.name = details.name || "PasskeyRegistrationError";
  return registrationError;
};

export const usePasskeyRegistration = () => {
  const registerPasskeyCredential = async (
    token: string,
    messages: RegisterPasskeyMessages,
  ) => {
    const loadCreationOptions = async (registrationProfile?: "standard") => {
      const optionsRes = await apiClient.post("/passkey/register/options", {
        token,
        registrationProfile,
      });
      return normalizeCreationOptions(optionsRes.data.data);
    };
    let creationOptions = await loadCreationOptions();
    let credential: Credential | null;
    try {
      credential = await navigator.credentials.create({
        publicKey: creationOptions,
      });
    } catch (error) {
      if (
        !shouldRetryPasskeyRegistrationWithStandardProfile(
          error,
          isAndroidPasskeyClient(),
        )
      ) {
        throw passkeyCreationError(error, creationOptions, messages);
      }

      console.info(
        "Android Passkey provider failed; retrying with the standard WebAuthn profile",
        getPasskeyErrorDetails(error),
      );
      creationOptions = await loadCreationOptions("standard");
      try {
        credential = await navigator.credentials.create({
          publicKey: creationOptions,
        });
      } catch (fallbackError) {
        throw passkeyCreationError(fallbackError, creationOptions, messages);
      }
    }
    if (!credential) {
      throw new Error(messages.noResponse);
    }

    const payload = serializeCredential(credential as PublicKeyCredential);
    const verifyRes = await apiClient.post("/passkey/register/verify", {
      token,
      deviceName: resolvePasskeyDeviceName(),
      credential: payload,
    });
    if (!verifyRes.data.success) {
      throw new Error(verifyRes.data.message || messages.bindFailed);
    }

    return {
      credentialId: payload.id,
    };
  };

  return {
    registerPasskeyCredential,
  };
};
