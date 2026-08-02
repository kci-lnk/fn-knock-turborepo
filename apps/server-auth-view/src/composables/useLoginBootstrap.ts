import { onBeforeUnmount, onMounted, type Ref } from "vue";
import type {
  AuthBootstrapData,
  AuthGrantType,
  AuthLdapProvider,
  AuthOidcProvider,
} from "@frontend-core/auth/types";
import type { CaptchaPublicSettings } from "@frontend-core/captcha/types";
import { AuthAPI } from "@/lib/api";

interface UseLoginBootstrapOptions {
  applyAuthSystemConfig: (data: AuthBootstrapData) => Promise<unknown>;
  bootstrapGrantType: Ref<AuthGrantType | undefined>;
  captchaConfig: Ref<CaptchaPublicSettings | null>;
  isCaptchaConfigLoading: Ref<boolean>;
  isPasskeyAvailable: Ref<boolean>;
  ldapProviderId: Ref<string>;
  ldapProviders: Ref<AuthLdapProvider[]>;
  loginMode: Ref<"totp" | "password">;
  navigateAfterBootstrap: (options: {
    authenticated: boolean;
    redirectTo?: string | null;
  }) => Promise<boolean>;
  oidcError: Ref<string>;
  oidcProviders: Ref<AuthOidcProvider[]>;
  redirectUri: string | null;
  refreshBrowserCapabilities: () => void;
  reportError: (message: string) => void;
  startLocationPolling: (client: AuthBootstrapData["client"]) => void;
  translate: (key: string) => string;
}

export function useLoginBootstrap({
  applyAuthSystemConfig,
  bootstrapGrantType,
  captchaConfig,
  isCaptchaConfigLoading,
  isPasskeyAvailable,
  ldapProviderId,
  ldapProviders,
  loginMode,
  navigateAfterBootstrap,
  oidcError,
  oidcProviders,
  redirectUri,
  refreshBrowserCapabilities,
  reportError,
  startLocationPolling,
  translate,
}: UseLoginBootstrapOptions) {
  let disposed = false;

  const loadBootstrap = async () => {
    try {
      const bootstrap = await AuthAPI.getBootstrap(redirectUri);
      if (disposed) return;
      await applyAuthSystemConfig(bootstrap);
      if (disposed) return;
      startLocationPolling(bootstrap.client);
      captchaConfig.value = bootstrap.captcha;
      isPasskeyAvailable.value = bootstrap.passkey.available;
      ldapProviders.value = bootstrap.ldap?.providers || [];
      if (
        !ldapProviders.value.some(
          (provider) => provider.id === ldapProviderId.value,
        )
      ) {
        ldapProviderId.value = ldapProviders.value[0]?.id || "";
      }
      oidcProviders.value = bootstrap.oidc?.providers || [];
      oidcError.value = bootstrap.oidc?.login_error || "";
      bootstrapGrantType.value = bootstrap.auth.grant_type;
      loginMode.value =
        bootstrap.auth.login_mode === "password" ? "password" : "totp";
      await navigateAfterBootstrap({
        authenticated: bootstrap.auth.authenticated,
        redirectTo: bootstrap.redirect_to,
      });
    } catch (error: any) {
      if (disposed) return;
      reportError(
        error?.response?.data?.message ||
          error?.message ||
          translate("auth.captchaConfigLoadFailed"),
      );
    } finally {
      if (!disposed) isCaptchaConfigLoading.value = false;
    }
  };

  onMounted(async () => {
    refreshBrowserCapabilities();
    await loadBootstrap();
  });
  onBeforeUnmount(() => {
    disposed = true;
  });

  return { loadBootstrap };
}
