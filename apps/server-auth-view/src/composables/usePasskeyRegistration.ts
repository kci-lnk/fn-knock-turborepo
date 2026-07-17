import {
  normalizeCreationOptions,
  serializeCredential,
} from "@frontend-core/passkey/utils";
import { apiClient } from "@/lib/api";
import {
  getPasskeyErrorDetails,
  resolvePasskeyRegistrationError,
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

export const usePasskeyRegistration = () => {
  const registerPasskeyCredential = async (
    token: string,
    messages: RegisterPasskeyMessages,
  ) => {
    const optionsRes = await apiClient.post("/passkey/register/options", {
      token,
    });
    const creationOptions = normalizeCreationOptions(optionsRes.data.data);
    let credential: Credential | null;
    try {
      credential = await navigator.credentials.create({
        publicKey: creationOptions,
      });
    } catch (error) {
      const details = getPasskeyErrorDetails(error);
      console.warn("Passkey credential creation failed", {
        name: details.name,
        message: details.message,
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
      throw registrationError;
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
