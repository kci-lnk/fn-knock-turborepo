<template>
  <AuthShell>
    <AuthCard
      :title="t('auth.title')"
      :description="
        isCaptchaVerified
          ? loginMode === 'password'
            ? t('auth.passwordPrompt')
            : t('auth.otpPrompt')
          : t('auth.captchaFirst')
      "
    >
      <template #header-extra>
        <div
          v-if="logoutNotice"
          class="mt-3 rounded-lg border border-border/70 bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
        >
          {{ logoutNotice }}
        </div>
        <div
          v-if="redirectGuardNotice"
          class="mt-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive"
          role="alert"
          aria-live="polite"
        >
          {{ redirectGuardNotice }}
        </div>
        <div
          v-if="oidcError"
          class="mt-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive"
          role="alert"
        >
          {{ oidcError }}
        </div>
      </template>

      <form
        class="flex flex-col gap-6 items-center"
        autocomplete="off"
        @submit.prevent="handleLogin"
      >
        <div
          v-if="
            !isCaptchaVerified &&
            activeCaptchaProvider === 'pow' &&
            isCaptchaProviderAvailable &&
            canUseNativePow
          "
          class="w-full flex justify-center mt-2"
        >
          <altcha-widget
            ref="powWidgetRef"
            :challengeurl="powChallengeUrl"
            :customfetch.prop="powChallengeFetch"
            @statechange="onPowStateChange"
            hidefooter
            hidelogo
            class="w-full"
            style="
              --altcha-color-border: pink;
              --altcha-border-width: 3px;
              --altcha-border-radius: 8px;
              --altcha-max-width: 360px;
            "
            :strings="powWidgetStrings"
          >
          </altcha-widget>
        </div>
        <div
          v-else-if="!isCaptchaVerified && isCaptchaConfigLoading"
          class="w-full mt-2 space-y-3"
        >
          <Skeleton class="h-11 w-full rounded-md" />
          <Skeleton class="h-4 w-2/3 rounded-md mx-auto" />
        </div>
        <div
          v-else-if="
            !isCaptchaVerified &&
            activeCaptchaProvider === 'pow' &&
            isCaptchaProviderAvailable
          "
          class="w-full mt-2 space-y-3"
        >
          <Button
            type="button"
            class="w-full"
            :disabled="isPowFallbackLoading"
            @click="handlePowFallbackVerify"
          >
            <span
              v-if="isPowFallbackLoading"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{
              isPowFallbackLoading ? t("auth.verifying") : t("auth.notRobot")
            }}
          </Button>
        </div>
        <div
          v-else-if="
            !isCaptchaVerified &&
            activeCaptchaProvider === 'turnstile' &&
            isCaptchaProviderAvailable
          "
          class="w-full mt-2 space-y-3"
        >
          <TurnstileWidget
            v-if="hasTurnstileSiteKey"
            ref="turnstileWidgetRef"
            :site-key="captchaConfig?.turnstile.site_key || ''"
            :disabled="isLoading || isPasskeyLoading"
            @verified="handleTurnstileVerified"
            @expired="handleCaptchaReset"
            @reset="handleCaptchaReset"
            @error="handleTurnstileError"
          />
          <div
            v-else
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
          >
            {{ t("auth.turnstileMissing") }}
          </div>
        </div>
        <div
          v-else-if="!isCaptchaVerified"
          class="w-full rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        >
          {{ captchaUnavailableReason }}
        </div>

        <div class="w-full" v-if="isPasskeySupported && isPasskeyAvailable">
          <Button
            type="button"
            :variant="isCaptchaVerified ? 'secondary' : 'default'"
            class="w-full"
            :disabled="isPasskeyLoading || isLoginCoolingDown"
            @click="handlePasskeyLogin"
          >
            <span
              v-if="isPasskeyLoading"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ passkeyButtonLabel }}
          </Button>
        </div>

        <div
          v-if="
            isPasskeySupported &&
            isPasskeyAvailable &&
            !isCaptchaVerified &&
            oidcProviders.length > 0
          "
          class="flex w-full items-center gap-3 text-sm text-muted-foreground"
          aria-hidden="true"
        >
          <div class="h-px flex-1 bg-border"></div>
          <span class="shrink-0">OR</span>
          <div class="h-px flex-1 bg-border"></div>
        </div>

        <OidcProviderButtons
          v-if="!isCaptchaVerified && oidcProviders.length > 0"
          :active-provider-id="activeOidcProviderId"
          :disabled="isOidcLoading || isLoginCoolingDown"
          :is-loading="isOidcLoading"
          :providers="oidcProviders"
          @login="handleOidcLogin"
        />

        <div
          v-if="isCaptchaVerified && loginMode === 'password'"
          class="w-full space-y-3"
        >
          <div class="space-y-2">
            <Label for="login-username">{{ t("auth.username") }}</Label>
            <Input
              id="login-username"
              v-model="username"
              autocomplete="username"
              :disabled="isLoading || isLoginCoolingDown"
            />
          </div>
          <div class="space-y-2">
            <Label for="login-password">{{ t("auth.password") }}</Label>
            <div class="relative">
              <Input
                id="login-password"
                v-model="password"
                :type="isPasswordVisible ? 'text' : 'password'"
                autocomplete="current-password"
                class="pr-10"
                :disabled="isLoading || isLoginCoolingDown"
              />
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                class="absolute right-1 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                :disabled="isLoading || isLoginCoolingDown"
                :title="
                  isPasswordVisible
                    ? t('auth.hidePassword')
                    : t('auth.showPassword')
                "
                :aria-label="
                  isPasswordVisible
                    ? t('auth.hidePassword')
                    : t('auth.showPassword')
                "
                @click="isPasswordVisible = !isPasswordVisible"
              >
                <component
                  :is="isPasswordVisible ? EyeOff : Eye"
                  class="h-4 w-4"
                />
              </Button>
            </div>
          </div>
        </div>

        <div
          class="w-full flex justify-center"
          v-if="isCaptchaVerified && loginMode === 'totp'"
        >
          <InputOTP
            :aria-label="t('auth.otpPrompt')"
            inputmode="numeric"
            :maxlength="6"
            v-model="token"
            @complete="handleOtpComplete"
            :disabled="isLoading || isLoginCoolingDown"
            autocomplete="one-time-code"
            data-form-type="other"
            data-1p-ignore="true"
            data-lpignore="true"
            data-bwignore="true"
          >
            <InputOTPGroup>
              <InputOTPSlot v-for="i in 6" :key="i - 1" :index="i - 1" />
            </InputOTPGroup>
          </InputOTP>
        </div>

        <Dialog :open="showErrorDialog" @update:open="showErrorDialog = $event">
          <DialogContent :show-close-button="false">
            <DialogHeader>
              <DialogTitle>{{ t("auth.tip") }}</DialogTitle>
              <DialogDescription>
                {{ errorMessage }}
              </DialogDescription>
            </DialogHeader>
            <DialogFooter>
              <Button @click="showErrorDialog = false">{{
                t("auth.ok")
              }}</Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
        <Dialog
          :open="showPasskeyBindDialog"
          @update:open="handlePasskeyBindDialogOpenChange"
        >
          <DialogContent
            :show-close-button="false"
            overlay-class="bg-black/50 backdrop-blur-sm"
          >
            <DialogHeader>
              <DialogTitle>{{ t("auth.passkeyBindTitle") }}</DialogTitle>
              <DialogDescription>
                {{ t("auth.passkeyBindDescription") }}
              </DialogDescription>
            </DialogHeader>
            <div
              v-if="passkeyBindError"
              class="text-sm text-destructive"
              role="alert"
            >
              {{ passkeyBindError }}
            </div>
            <div
              class="flex items-center space-x-3 rounded-lg border bg-muted/40 px-3 py-2"
            >
              <Checkbox
                id="skipPasskeyBindPrompt"
                v-model="skipPasskeyBindPrompt"
                :disabled="isBindingPasskey"
              />
              <label
                for="skipPasskeyBindPrompt"
                class="cursor-pointer select-none text-sm text-muted-foreground"
              >
                {{ t("auth.passkeyBindSkipPrompt") }}
              </label>
            </div>
            <DialogFooter class="gap-2">
              <Button variant="outline" @click="skipPasskeyBind">{{
                t("auth.passkeyBindLater")
              }}</Button>
              <Button :disabled="isBindingPasskey" @click="handlePasskeyBind">
                <span
                  v-if="isBindingPasskey"
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                ></span>
                {{ t("auth.passkeyBindNow") }}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
        <Button
          type="submit"
          class="w-full"
          :disabled="isLoading || isLoginCoolingDown"
          v-if="isCaptchaVerified"
          @click="handleLogin"
        >
          <span
            v-if="isLoading"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ loginButtonLabel }}
        </Button>

        <div
          class="w-full flex justify-center"
          v-if="isCaptchaVerified || oidcProviders.length > 0"
        >
          <div
            class="flex items-center justify-center space-x-3 py-2 px-4 rounded-lg transition-colors hover:bg-muted/50 cursor-pointer group"
          >
            <Checkbox
              id="rememberMe"
              v-model="rememberMe"
              class="data-[state=checked]:bg-primary data-[state=checked]:border-primary"
            />
            <label
              for="rememberMe"
              class="text-sm font-medium leading-none cursor-pointer select-none text-muted-foreground group-hover:text-foreground transition-colors"
            >
              {{ t("auth.rememberMe") }}
            </label>
          </div>
        </div>
      </form>
    </AuthCard>

    <template #footer>
      <AuthFooter
        :client-ip="clientIp"
        :ip-location="ipLocation"
        :ip-location-status="ipLocationStatus"
      />
    </template>
  </AuthShell>
</template>

<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Eye, EyeOff } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from "@/components/ui/input-otp";
import { Label } from "@/components/ui/label";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import type { AuthOidcProvider } from "@frontend-core/auth/types";
import { buildAuthApiPath, fetchNoStore } from "@/lib/api";
import { useClientIpLocation } from "@/lib/client-ip-location";
import AuthFooter from "@/components/AuthFooter.vue";
import AuthCard from "@/components/AuthCard.vue";
import AuthShell from "@/components/AuthShell.vue";
import OidcProviderButtons from "@/components/OidcProviderButtons.vue";
import TurnstileWidget from "@/components/captcha/TurnstileWidget.vue";
import { useAuthBrowserCapabilities } from "@/composables/useAuthBrowserCapabilities";
import { useAuthSystemConfig } from "@/composables/useAuthSystemConfig";
import { useLoginCaptcha } from "@/composables/useLoginCaptcha";
import { useLoginCooldown } from "@/composables/useLoginCooldown";
import { useLoginPasskey } from "@/composables/useLoginPasskey";
import { useLoginRedirect } from "@/composables/useLoginRedirect";
import { useCredentialLogin } from "@/composables/useCredentialLogin";
import { useLoginBootstrap } from "@/composables/useLoginBootstrap";
import { useOidcLogin } from "@/composables/useOidcLogin";

import "altcha";

const i18n = useI18n();
const { t } = i18n;
const {
  bootstrapGrantType,
  completeLogin,
  logoutNotice,
  navigateAfterBootstrap,
  redirectGuardNotice,
  redirectUri,
  resetRedirectGuard,
} = useLoginRedirect({
  translate: (key) => t(key),
});

const token = ref("");
const username = ref("");
const password = ref("");
const isPasswordVisible = ref(false);
const rememberMe = ref(false);
const errorMessage = ref("");
const showErrorDialog = ref(false);
const isPasskeyAvailable = ref(false);
const oidcProviders = ref<AuthOidcProvider[]>([]);
const oidcError = ref("");
const loginMode = ref<"totp" | "password">("totp");
const { clientIp, ipLocation, ipLocationStatus, startLocationPolling } =
  useClientIpLocation();

const reportLoginError = (message: string) => {
  errorMessage.value = message;
  showErrorDialog.value = true;
};

const { canUseNativePow, isPasskeySupported, refreshBrowserCapabilities } =
  useAuthBrowserCapabilities();
const { applyAuthSystemConfig } = useAuthSystemConfig(i18n);
const {
  activeCaptchaProvider,
  captchaConfig,
  captchaSubmission,
  captchaUnavailableReason,
  handlePowFallbackVerify,
  handlePowStateChange: onPowStateChange,
  handleTurnstileError,
  handleTurnstileVerified,
  hasTurnstileSiteKey,
  isCaptchaConfigLoading,
  isCaptchaProviderAvailable,
  isCaptchaVerified,
  isPowFallbackLoading,
  powWidgetRef,
  powWidgetStrings,
  resetCaptcha: handleCaptchaReset,
  resetCaptchaWidgets,
  turnstileWidgetRef,
} = useLoginCaptcha({
  canUseNativePow,
  translate: (key) => t(key),
  onError: reportLoginError,
  onVerified: () => {
    errorMessage.value = "";
  },
});
// Vue assigns string template refs at runtime; keep them visible to TypeScript.
void powWidgetRef;
void turnstileWidgetRef;
const {
  isCoolingDown: isLoginCoolingDown,
  remainingSeconds: loginCooldownSeconds,
  resolveMessage: resolveLoginCooldownMessage,
} = useLoginCooldown({
  formatRetrySuffix: (seconds) => t("auth.retrySuffix", { seconds }),
});
const {
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
} = useLoginPasskey({
  clearError: () => {
    errorMessage.value = "";
  },
  completeLogin,
  isLoginCoolingDown,
  isPasskeyAvailable,
  isPasskeySupported,
  redirectUri,
  rememberMe,
  reportError: reportLoginError,
  resolveLoginCooldownMessage,
  translate: (key) => t(key),
});

const powChallengeUrl = buildAuthApiPath("/challenge");
const powChallengeFetch = (input: string | URL, init?: RequestInit) =>
  fetchNoStore(input, init);
const { handleLogin, handleOtpComplete, isLoading, loginButtonLabel } =
  useCredentialLogin({
    captchaSubmission,
    clearError: () => {
      errorMessage.value = "";
    },
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
    reportError: reportLoginError,
    resetCaptchaWidgets,
    resolveLoginCooldownMessage,
    token,
    translate: (key, params) => (params ? t(key, params) : t(key)),
    username,
  });

const { activeOidcProviderId, handleOidcLogin, isOidcLoading } = useOidcLogin({
  clearError: () => {
    errorMessage.value = "";
  },
  isLoginCoolingDown,
  redirectUri,
  rememberMe,
  reportError: reportLoginError,
  resetRedirectGuard,
  translate: (key) => t(key),
});

useLoginBootstrap({
  applyAuthSystemConfig,
  bootstrapGrantType,
  captchaConfig,
  isCaptchaConfigLoading,
  isPasskeyAvailable,
  loginMode,
  navigateAfterBootstrap,
  oidcError,
  oidcProviders,
  redirectUri,
  refreshBrowserCapabilities,
  reportError: reportLoginError,
  startLocationPolling,
  translate: (key) => t(key),
});

const passkeyButtonLabel = computed(() => {
  if (isPasskeyLoading.value) {
    return t("auth.verifying");
  }
  if (isLoginCoolingDown.value) {
    return t("auth.retryAfterSeconds", {
      seconds: loginCooldownSeconds.value,
    });
  }
  return t("auth.passkeyLogin");
});
</script>
