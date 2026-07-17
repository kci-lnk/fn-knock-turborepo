import { computed, ref } from "vue";
import { useRouter } from "vue-router";
import type { AuthGrantType } from "@frontend-core/auth/types";
import {
  guardAuthAutoRedirect,
  resetAuthAutoRedirectGuard,
  type AuthRedirectBlockReason,
  type RedirectGuardStorage,
} from "@/lib/auth-redirect-guard";
import { markPendingLogoutDelay } from "@/lib/post-login";

export const useLoginRedirect = ({
  translate,
}: {
  translate: (key: string) => string;
}) => {
  const router = useRouter();
  const queryParams =
    typeof window !== "undefined"
      ? new URLSearchParams(window.location.search)
      : null;
  const redirectUri = queryParams?.get("redirect_uri") ?? null;
  const suppressAutoRedirect = queryParams?.get("logged_out") === "1";
  const bootstrapGrantType = ref<AuthGrantType | undefined>();
  const redirectGuardBlockReason = ref<AuthRedirectBlockReason | null>(null);

  const logoutNotice = computed(() => {
    if (!suppressAutoRedirect) return "";
    switch (bootstrapGrantType.value) {
      case "login_ip_grant":
        return translate("auth.loggedOutLoginIpGrant");
      case "manual_whitelist":
        return translate("auth.loggedOutManualWhitelist");
      case "local_exempt":
        return translate("auth.loggedOutLocalExempt");
      default:
        return translate("auth.loggedOutDefault");
    }
  });

  const redirectGuardNotice = computed(() => {
    if (!redirectGuardBlockReason.value) return "";
    return redirectGuardBlockReason.value === "repeat_redirect"
      ? translate("auth.redirectLoopBlocked")
      : translate("auth.redirectTargetBlocked");
  });

  const getRedirectGuardStorage = (): RedirectGuardStorage | null => {
    if (typeof window === "undefined") return null;
    try {
      return window.sessionStorage;
    } catch {
      return null;
    }
  };

  const resetRedirectGuard = () => {
    resetAuthAutoRedirectGuard(getRedirectGuardStorage());
    redirectGuardBlockReason.value = null;
  };

  const navigateAfterBootstrap = async ({
    authenticated,
    redirectTo,
  }: {
    authenticated: boolean;
    redirectTo?: string | null;
  }) => {
    if (suppressAutoRedirect) return false;

    if (redirectTo) {
      const decision = guardAuthAutoRedirect({
        redirectTo,
        currentUrl: window.location.href,
        storage: getRedirectGuardStorage(),
      });
      if (!decision.allowed) {
        redirectGuardBlockReason.value = decision.reason;
        return true;
      }
      window.location.replace(decision.redirectUrl);
      return true;
    }

    if (authenticated) {
      await router.replace("/");
      return true;
    }
    return false;
  };

  const completeLogin = (runType: 0 | 1 | 3, redirectTo?: string | null) => {
    resetRedirectGuard();
    markPendingLogoutDelay();
    if (redirectTo) {
      const decision = guardAuthAutoRedirect({
        redirectTo,
        currentUrl: window.location.href,
        storage: getRedirectGuardStorage(),
      });
      if (!decision.allowed) {
        redirectGuardBlockReason.value = decision.reason;
        return;
      }
      window.location.replace(decision.redirectUrl);
      return;
    }
    if (runType === 0) {
      void router.replace("/");
    } else {
      window.location.replace("/");
    }
  };

  return {
    bootstrapGrantType,
    completeLogin,
    logoutNotice,
    navigateAfterBootstrap,
    redirectGuardNotice,
    redirectUri,
    resetRedirectGuard,
    suppressAutoRedirect,
  };
};
