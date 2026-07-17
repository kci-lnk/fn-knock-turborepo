export const shouldOfferPasskeyBinding = ({
  canBindPasskey,
  currentBrowserHasKnownPasskey,
  isPasskeySupported,
  loginMode,
}: {
  canBindPasskey: boolean;
  currentBrowserHasKnownPasskey: boolean;
  isPasskeySupported: boolean;
  loginMode?: "totp" | "password";
}) =>
  canBindPasskey &&
  !currentBrowserHasKnownPasskey &&
  isPasskeySupported &&
  loginMode !== "password";

export const passkeyBindingCopyKeys = (accountHasPasskey: boolean) =>
  accountHasPasskey
    ? {
        button: "auth.home.addPasskey",
        hint: "auth.home.passkeyAvailableAddDevice",
      }
    : {
        button: "auth.home.enablePasskey",
        hint: "auth.home.passkeySupportedUnbound",
      };
