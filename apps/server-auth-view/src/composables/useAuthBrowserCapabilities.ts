import { ref } from "vue";

export const useAuthBrowserCapabilities = () => {
  const isPasskeySupported = ref(false);
  const canUseNativePow = ref(true);

  const refreshBrowserCapabilities = () => {
    isPasskeySupported.value =
      typeof window !== "undefined" &&
      window.isSecureContext &&
      typeof window.PublicKeyCredential === "function" &&
      typeof navigator !== "undefined" &&
      typeof navigator.credentials?.create === "function" &&
      typeof navigator.credentials?.get === "function";
    canUseNativePow.value =
      typeof window !== "undefined" &&
      window.isSecureContext &&
      typeof window.crypto !== "undefined" &&
      !!window.crypto.subtle &&
      typeof window.crypto.subtle.digest === "function";
  };

  return {
    canUseNativePow,
    isPasskeySupported,
    refreshBrowserCapabilities,
  };
};
