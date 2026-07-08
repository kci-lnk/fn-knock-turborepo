import {
  normalizeCreationOptions,
  serializeCredential,
} from "@frontend-core/passkey/utils";
import { apiClient } from "@/lib/api";

type RegisterPasskeyMessages = {
  bindFailed: string;
  noResponse: string;
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
    const credential = await navigator.credentials.create({
      publicKey: creationOptions,
    });
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
