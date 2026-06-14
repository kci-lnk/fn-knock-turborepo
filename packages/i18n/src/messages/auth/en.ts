export const enAuth = {
  autoIpGrantComment: "Automatically authorized after sign-in",
  title: "Security verification",
  captchaFirst: "Complete the human verification below first",
  otpPrompt: "Enter your six-digit one-time password to sign in",
  notRobot: "I'm not a robot",
  verified: "Verified",
  verifying: "Verifying...",
  wait: "Please wait...",
  verifyError: "Verification error",
  turnstileMissing:
    "Turnstile is not configured. Ask an administrator to set the site key.",
  turnstileScriptLoadFailed: "Failed to load the Turnstile script",
  turnstileRenderFailed: "Turnstile failed to render. Try again later.",
  turnstileTimeout: "Turnstile verification timed out. Try again.",
  powUnsupportedAlgorithm: "Unsupported PoW algorithm",
  powInvalidChallenge: "Invalid PoW challenge data",
  powSolveFailed: "PoW solving failed. Refresh the page and try again.",
  locationResolving: "Resolving location...",
  locationUnavailable: "Location unavailable",
  openGithub: "Open GitHub project page",
  menu: "Menu",
  or: "OR",
  loginWithProvider: "Sign in with {provider}",
  retryAfterSeconds: "Retry in {seconds}s",
  verifyNow: "Verify now",
  passkeyLogin: "Sign in with Passkey",
  tip: "Notice",
  ok: "OK",
  rememberMe: "Remember me",
  passkeyBindTitle: "Enable Passkey sign-in",
  passkeyBindDescription:
    "Bind a Passkey on this device so you can sign in with one action next time.",
  passkeyBindSkipPrompt: "Don't remind me again",
  passkeyBindLater: "Maybe later",
  passkeyBindNow: "Enable now",
  captchaConfigLoadFailed:
    "Failed to load captcha configuration. Refresh the page and try again.",
  captchaFailed: "Human verification failed. Please try again.",
  loggedOutLoginIpGrant:
    "Your browser session has signed out. The IP access granted at sign-in has also been revoked.",
  loggedOutManualWhitelist:
    "Your browser session has signed out. The administrator whitelist is still active.",
  loggedOutLocalExempt:
    "Your browser session has signed out. This network is still exempt from whitelist checks.",
  loggedOutDefault: "Your browser session has signed out. Verify again.",
  retrySuffix: " Retry in {seconds} seconds.",
  invalidOtpLength: "Enter the complete 6-digit verification code",
  loginFailed: "Verification failed. Please try again.",
  passkeyNoResponse: "No Passkey response was returned",
  passkeyVerifyFailed: "Passkey verification failed",
  passkeyLoginFailed: "Passkey sign-in failed. Please try again.",
  oidcStartFailed: "Unable to start external sign-in",
  oidcLoginFailed: "External sign-in failed. Please try again.",
  passkeyBindInvalid: "Binding credential is invalid. Sign in again.",
  passkeyBindFailed: "Passkey binding failed",
  home: {
    statusTitles: {
      browserSession: "This browser session is verified",
      sessionMigration: "Browser session restored",
      fnosFingerprintSession: "Device fingerprint session restored",
      manualWhitelist: "Whitelist access allowed",
      localExempt: "Current network allowed",
      fnosShare: "Share access authorized",
      loginIpGrant: "Security verification passed",
    },
    statusDescriptions: {
      browserSession: "This browser session is allowed to access",
      sessionMigration:
        "This browser session was restored after a network change",
      fnosFingerprintSession:
        "This access was restored by a FNOS device fingerprint session",
      manualWhitelist: "The current IP is on the administrator whitelist",
      localExempt: "This network address is exempt from whitelist checks",
      fnosShare: "This access was authorized by a FNOS share link",
      loginIpGrant: "Your IP has been authorized for access",
    },
    logoutHints: {
      browserSession:
        "When you no longer need access, sign out below. This browser must verify again before entering.",
      sessionMigration:
        "When you no longer need access, sign out below. This browser must verify again, and the authorization tied to this session migration will be revoked.",
      fnosFingerprintSession:
        "When you no longer need access, sign out below. The restored device fingerprint session will end and its linked authorization will be revoked.",
      loginIpGrant:
        "When you no longer need access, sign out below. This browser session will end, and the current IP access granted at sign-in will also be revoked.",
      manualWhitelist:
        "When you no longer need access, sign out below. Only this browser session will end; the administrator whitelist will remain.",
      localExempt:
        "When you no longer need access, sign out below. Only this browser session will end; this network's whitelist exemption will not change.",
      fnosShare:
        "When you no longer need access, sign out below. This share access session will end, and you will need to open the share link again.",
      default:
        "When you no longer need access, sign out below and revoke your authorization.",
    },
    logoutDialogDescriptions: {
      browserSession:
        "Signing out will end this browser session. You must verify again before entering.",
      sessionMigration:
        "Signing out will end this browser session and revoke the authorization tied to this session migration.",
      fnosFingerprintSession:
        "Signing out will end the restored device fingerprint session and revoke its linked authorization.",
      loginIpGrant:
        "Signing out will end this browser session and revoke the current IP access granted by this sign-in.",
      manualWhitelist:
        "Signing out only ends this browser session. The administrator whitelist will remain.",
      localExempt:
        "Signing out only ends this browser session. This network's whitelist exemption will not change.",
      fnosShare:
        "Signing out will end this share access session. Open the share link again to access it later.",
      default:
        "Signing out will revoke the current access authorization. You must verify again before entering.",
    },
    enablePasskey: "Enable Passkey sign-in",
    passkeySupportedUnbound:
      "This browser supports Passkey, but none is bound yet",
    logoutDelay: "The sign-out button will appear in {seconds} seconds",
    logout: "Sign out",
    logoutConfirmTitle: "Confirm sign out",
    confirmLogout: "Sign out",
    passkeyTokenMissing: "Unable to get binding credential",
  },
  oidcBind: {
    title: "Bind external account",
    checkingInvite: "Checking invite link...",
    bindTo: "Bind to",
    useProvider: "Bind with {provider}",
    invalidInvite: "Invite link unavailable",
    wait: "Please wait",
    selectProvider: "Choose a provider to sign in and bind",
    missingToken: "Invite link is missing token",
    noProviders: "No external sign-in providers are available",
    inviteExpired: "Invite link has expired",
    startFailed: "Unable to start external account binding",
    bindFailed: "External account binding failed. Please try again.",
  },
};
