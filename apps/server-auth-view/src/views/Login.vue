<template>
  <div class="auth-safe-shell auth-visual-shell flex flex-col">
    <div class="flex flex-1 items-center justify-center">
      <Card class="auth-glass-card w-full max-w-sm">
        <CardHeader>
          <CardTitle class="text-2xl text-center">{{ t("auth.title") }}</CardTitle>
          <CardDescription class="text-center" v-if="!isCaptchaVerified">
            {{ t("auth.captchaFirst") }}
          </CardDescription>
          <CardDescription class="text-center" v-else>
            {{ t("auth.otpPrompt") }}
          </CardDescription>
          <div
            v-if="logoutNotice"
            class="mt-3 rounded-lg border border-border/70 bg-muted/50 px-3 py-2 text-sm text-muted-foreground"
          >
            {{ logoutNotice }}
          </div>
          <div
            v-if="oidcError"
            class="mt-3 rounded-lg border border-destructive/30 bg-destructive/5 px-3 py-2 text-sm text-destructive"
          >
            {{ oidcError }}
          </div>
        </CardHeader>

        <CardContent>
          <form class="flex flex-col gap-6 items-center" autocomplete="off">
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
                :strings="
                  powWidgetStrings
                "
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
                {{ isPowFallbackLoading ? t("auth.verifying") : t("auth.notRobot") }}
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

            <div
              v-if="!isCaptchaVerified && oidcProviders.length > 0"
              class="w-full space-y-2"
            >
              <Button
                v-for="provider in oidcProviders"
                :key="provider.id"
                type="button"
                variant="outline"
                class="w-full"
                :disabled="isOidcLoading || isLoginCoolingDown"
                @click="handleOidcLogin(provider.id)"
              >
                <span
                  v-if="activeOidcProviderId === provider.id && isOidcLoading"
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
                ></span>
                <Github
                  v-else-if="providerIconKind(provider) === 'github'"
                  class="size-4"
                  aria-hidden="true"
                />
                <svg
                  v-else-if="providerIconKind(provider) === 'google'"
                  class="size-4"
                  viewBox="0 0 24 24"
                  aria-hidden="true"
                >
                  <path
                    fill="#4285F4"
                    d="M23.77 12.28c0-.82-.07-1.63-.21-2.44H12.24v4.62h6.48a5.54 5.54 0 0 1-2.4 3.64v3.02h3.89c2.28-2.1 3.56-5.19 3.56-8.84Z"
                  />
                  <path
                    fill="#34A853"
                    d="M12.24 24c3.24 0 5.97-1.06 7.95-2.88L16.3 18.1c-1.08.73-2.47 1.15-4.06 1.15-3.13 0-5.78-2.11-6.73-4.95H1.49v3.11A12 12 0 0 0 12.24 24Z"
                  />
                  <path
                    fill="#FBBC05"
                    d="M5.51 14.3a7.19 7.19 0 0 1 0-4.6V6.59H1.49a12.01 12.01 0 0 0 0 10.82L5.51 14.3Z"
                  />
                  <path
                    fill="#EA4335"
                    d="M12.24 4.75a6.52 6.52 0 0 1 4.6 1.8l3.45-3.45A11.58 11.58 0 0 0 12.24 0 12 12 0 0 0 1.49 6.59L5.51 9.7c.95-2.84 3.6-4.95 6.73-4.95Z"
                  />
                </svg>
                <span
                  v-else-if="providerIconKind(provider) === 'microsoft'"
                  class="grid size-4 grid-cols-2 gap-0.5"
                  aria-hidden="true"
                >
                  <span class="bg-[#f25022]"></span>
                  <span class="bg-[#7fba00]"></span>
                  <span class="bg-[#00a4ef]"></span>
                  <span class="bg-[#ffb900]"></span>
                </span>
                <Cloud
                  v-else-if="providerIconKind(provider) === 'custom_oidc'"
                  class="size-4"
                  aria-hidden="true"
                />
                <CircleUserRound v-else class="size-4" aria-hidden="true" />
                {{ t("auth.loginWithProvider", { provider: provider.name }) }}
              </Button>
            </div>

            <div class="w-full flex justify-center" v-if="isCaptchaVerified">
              <InputOTP
                inputmode="numeric"
                :maxlength="6"
                v-model="token"
                @complete="handleOtpComplete"
                :disabled="isLoading || isLoginCoolingDown"
                :autofocus="true"
                autocomplete="off"
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

            <Dialog
              :open="showErrorDialog"
              @update:open="showErrorDialog = $event"
            >
              <DialogContent :show-close-button="false">
                <DialogHeader>
                  <DialogTitle>{{ t("auth.tip") }}</DialogTitle>
                  <DialogDescription>
                    {{ errorMessage }}
                  </DialogDescription>
                </DialogHeader>
                <DialogFooter>
                  <Button @click="showErrorDialog = false">{{ t("auth.ok") }}</Button>
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
                <div v-if="passkeyBindError" class="text-sm text-destructive">
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
                  <Button variant="outline" @click="skipPasskeyBind"
                    >{{ t("auth.passkeyBindLater") }}</Button
                  >
                  <Button
                    :disabled="isBindingPasskey"
                    @click="handlePasskeyBind"
                  >
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
              type="button"
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
        </CardContent>
      </Card>
    </div>

    <AuthFooter
      :client-ip="clientIp"
      :ip-location="ipLocation"
      :ip-location-status="ipLocationStatus"
    />
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  CircleUserRound,
  Cloud,
  Github,
} from "lucide-vue-next";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  InputOTP,
  InputOTPGroup,
  InputOTPSlot,
} from "@/components/ui/input-otp";
import { Skeleton } from "@/components/ui/skeleton";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  normalizeCreationOptions,
  normalizeRequestOptions,
  serializeCredential,
} from "@frontend-core/passkey/utils";
import type {
  AuthGrantType,
  AuthOidcProvider,
} from "@frontend-core/auth/types";
import type {
  CaptchaPublicSettings,
  CaptchaSubmission,
} from "@frontend-core/captcha/types";
import {
  apiClient,
  AuthAPI,
  buildAuthApiPath,
  CaptchaAPI,
  fetchNoStore,
} from "@/lib/api";
import { useClientIpLocation } from "@/lib/client-ip-location";
import {
  buildPowSubmission,
  CaptchaError,
  normalizePowChallenge,
  solvePowChallenge,
} from "@/lib/captcha";
import { markPendingLogoutDelay } from "@/lib/post-login";
import AuthFooter from "@/components/AuthFooter.vue";
import TurnstileWidget from "@/components/captcha/TurnstileWidget.vue";
import { setFnKnockLocale } from "@fn-knock/i18n/vue/auth";

import "altcha";

const router = useRouter();
const i18n = useI18n();
const { t } = i18n;

const token = ref("");
const rememberMe = ref(false);
const errorMessage = ref("");
const showErrorDialog = ref(false);
const isLoading = ref(false);
const loginCooldownSeconds = ref(0);
const isPasskeySupported = ref(false);
const isPasskeyAvailable = ref(false);
const isPasskeyLoading = ref(false);
const isOidcLoading = ref(false);
const activeOidcProviderId = ref("");
const oidcProviders = ref<AuthOidcProvider[]>([]);
const oidcError = ref("");
const showPasskeyBindDialog = ref(false);
const isBindingPasskey = ref(false);
const passkeyBindError = ref("");
const passkeyBindToken = ref("");
const skipPasskeyBindPrompt = ref(false);
const pendingRunType = ref<0 | 1 | 3 | null>(null);
const pendingRedirectTo = ref<string | null>(null);
const { clientIp, ipLocation, ipLocationStatus, startLocationPolling } =
  useClientIpLocation();
let lastLoginAttemptAt = 0;
let loginCooldownTimer: number | null = null;
const PASSKEY_BIND_PROMPT_STORAGE_KEY =
  "server-auth-view:passkey-bind-prompt-dismissed";
const PASSKEY_CREDENTIAL_IDS_STORAGE_KEY =
  "server-auth-view:known-passkey-credential-digests";

const captchaConfig = ref<CaptchaPublicSettings | null>(null);
const powWidgetRef = ref<any>(null);
const turnstileWidgetRef = ref<InstanceType<typeof TurnstileWidget> | null>(
  null,
);
const isCaptchaVerified = ref(false);
const captchaSubmission = ref<CaptchaSubmission | null>(null);
const canUseNativePow = ref(true);
const isPowFallbackLoading = ref(false);
const isCaptchaConfigLoading = ref(true);

const powChallengeUrl = buildAuthApiPath("/challenge");
const powChallengeFetch = (input: string | URL, init?: RequestInit) =>
  fetchNoStore(input, init);
const activeCaptchaProvider = computed(
  () => captchaConfig.value?.provider ?? null,
);
const isCaptchaProviderAvailable = computed(
  () => captchaConfig.value?.available ?? false,
);
const captchaUnavailableReason = computed(
  () =>
    captchaConfig.value?.unavailable_reason ||
    t("auth.captchaConfigLoadFailed"),
);
const hasTurnstileSiteKey = computed(
  () => !!captchaConfig.value?.turnstile.site_key.trim(),
);
const isLoginCoolingDown = computed(() => loginCooldownSeconds.value > 0);
const applySystemLocale = async (value: string | null | undefined) => {
  await setFnKnockLocale(i18n, value);
};
const powWidgetStrings = computed(() =>
  JSON.stringify({
    label: t("auth.notRobot"),
    verified: t("auth.verified"),
    verifying: t("auth.verifying"),
    wait: t("auth.wait"),
    error: t("auth.verifyError"),
  }),
);
const loginButtonLabel = computed(() => {
  if (isLoading.value) {
    return t("auth.verifying");
  }
  if (isLoginCoolingDown.value) {
    return t("auth.retryAfterSeconds", {
      seconds: loginCooldownSeconds.value,
    });
  }
  return t("auth.verifyNow");
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
const queryParams =
  typeof window !== "undefined"
    ? new URLSearchParams(window.location.search)
    : null;
const redirectUri = queryParams?.get("redirect_uri") ?? null;
const suppressAutoRedirect = queryParams?.get("logged_out") === "1";
const bootstrapGrantType = ref<AuthGrantType | undefined>(undefined);
const logoutNotice = computed(() => {
  if (!suppressAutoRedirect) {
    return "";
  }

  switch (bootstrapGrantType.value) {
    case "login_ip_grant":
      return t("auth.loggedOutLoginIpGrant");
    case "manual_whitelist":
      return t("auth.loggedOutManualWhitelist");
    case "local_exempt":
      return t("auth.loggedOutLocalExempt");
    default:
      return t("auth.loggedOutDefault");
  }
});

type ProviderIconKind =
  | "github"
  | "google"
  | "microsoft"
  | "custom_oidc"
  | "generic";

function providerIconKind(provider: AuthOidcProvider): ProviderIconKind {
  const token = `${provider.type || ""} ${provider.name || ""} ${
    provider.protocol || ""
  }`.toLowerCase();
  if (token.includes("github")) return "github";
  if (token.includes("google")) return "google";
  if (token.includes("microsoft") || token.includes("azure")) {
    return "microsoft";
  }
  if (token.includes("custom") || token.includes("oidc")) return "custom_oidc";
  return "generic";
}

function onPowStateChange(ev: CustomEvent) {
  if (ev.detail.state === "verified") {
    isCaptchaVerified.value = true;
    captchaSubmission.value = {
      provider: "pow",
      proof: ev.detail.payload,
    };
    errorMessage.value = "";
  } else {
    handleCaptchaReset();
  }
}

onMounted(async () => {
  initBrowserCapabilities();
  await loadBootstrap();
});

onUnmounted(() => {
  clearLoginCooldownTimer();
});

function initBrowserCapabilities() {
  isPasskeySupported.value =
    typeof window !== "undefined" && !!window.PublicKeyCredential;
  canUseNativePow.value =
    typeof window !== "undefined" &&
    window.isSecureContext &&
    typeof window.crypto !== "undefined" &&
    !!window.crypto.subtle &&
    typeof window.crypto.subtle.digest === "function";
}

async function loadBootstrap() {
  try {
    const bootstrap = await AuthAPI.getBootstrap(redirectUri);
    await applySystemLocale(bootstrap.locale.default_locale);
    startLocationPolling(bootstrap.client);
    captchaConfig.value = bootstrap.captcha;
    isPasskeyAvailable.value = !!bootstrap.passkey.available;
    oidcProviders.value = bootstrap.oidc?.providers || [];
    oidcError.value = bootstrap.oidc?.login_error || "";
    bootstrapGrantType.value = bootstrap.auth.grant_type;
    if (bootstrap.redirect_to && !suppressAutoRedirect) {
      window.location.replace(bootstrap.redirect_to);
      return;
    }
    if (bootstrap.auth.authenticated && !suppressAutoRedirect) {
      await router.replace("/");
      return;
    }
  } catch (e: any) {
    errorMessage.value =
      e?.response?.data?.message ||
      e?.message ||
      t("auth.captchaConfigLoadFailed");
    showErrorDialog.value = true;
  } finally {
    isCaptchaConfigLoading.value = false;
  }
}

async function handlePowFallbackVerify() {
  if (isPowFallbackLoading.value) return;
  isPowFallbackLoading.value = true;
  errorMessage.value = "";
  try {
    const challenge = normalizePowChallenge(await CaptchaAPI.getPowChallenge());
    const number = await solvePowChallenge(challenge);
    captchaSubmission.value = buildPowSubmission(challenge, number);
    isCaptchaVerified.value = true;
  } catch (e: any) {
    handleCaptchaReset();
    errorMessage.value =
      e instanceof CaptchaError
        ? t(`auth.${e.code}`)
        : e?.response?.data?.message || e?.message || t("auth.captchaFailed");
    showErrorDialog.value = true;
  } finally {
    isPowFallbackLoading.value = false;
  }
}

function handleTurnstileVerified(token: string) {
  isCaptchaVerified.value = true;
  captchaSubmission.value = {
    provider: "turnstile",
    token,
  };
  errorMessage.value = "";
}

function handleTurnstileError(message: string) {
  handleCaptchaReset();
  errorMessage.value = message;
  showErrorDialog.value = true;
}

function handleCaptchaReset() {
  isCaptchaVerified.value = false;
  captchaSubmission.value = null;
}

function clearLoginCooldownTimer() {
  if (typeof window === "undefined" || loginCooldownTimer === null) {
    return;
  }

  window.clearInterval(loginCooldownTimer);
  loginCooldownTimer = null;
}

function startLoginCooldown(seconds: unknown) {
  const parsedSeconds = Math.max(0, Math.ceil(Number(seconds) || 0));
  if (parsedSeconds <= 0) {
    return 0;
  }

  clearLoginCooldownTimer();
  loginCooldownSeconds.value = parsedSeconds;

  if (typeof window === "undefined") {
    return parsedSeconds;
  }

  loginCooldownTimer = window.setInterval(() => {
    if (loginCooldownSeconds.value <= 1) {
      loginCooldownSeconds.value = 0;
      clearLoginCooldownTimer();
      return;
    }
    loginCooldownSeconds.value -= 1;
  }, 1000);

  return parsedSeconds;
}

function extractRetryAfterSeconds(payload: any): number {
  const rawRetryAfter =
    payload?.retryAfter ??
    payload?.response?.data?.retryAfter ??
    payload?.response?.headers?.["retry-after"];
  const retryAfterValue = Array.isArray(rawRetryAfter)
    ? rawRetryAfter[0]
    : rawRetryAfter;
  const parsedSeconds = Number(retryAfterValue);

  return Number.isFinite(parsedSeconds) && parsedSeconds > 0
    ? Math.ceil(parsedSeconds)
    : 0;
}

function resolveRetryAfterMessage(message: string, retryAfter: number) {
  const retrySuffix = t("auth.retrySuffix", { seconds: retryAfter });
  if (
    retryAfter <= 0 ||
    message.includes(String(retryAfter)) ||
    message.includes(retrySuffix)
  ) {
    return message;
  }
  return `${message}${retrySuffix}`;
}

function handleOtpComplete() {
  void handleLogin();
}

function isPasskeyBindPromptDismissed() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(PASSKEY_BIND_PROMPT_STORAGE_KEY) === "1";
}

function normalizePasskeyCredentialIds(value: unknown): string[] {
  if (!Array.isArray(value)) {
    return [];
  }

  return [...new Set(value)]
    .filter((item): item is string => typeof item === "string")
    .map((item) => item.trim())
    .filter(Boolean);
}

function readKnownPasskeyCredentialDigests() {
  if (typeof window === "undefined") {
    return [] as string[];
  }

  try {
    const raw = window.localStorage.getItem(PASSKEY_CREDENTIAL_IDS_STORAGE_KEY);
    if (!raw) {
      return [] as string[];
    }

    return normalizePasskeyCredentialIds(JSON.parse(raw));
  } catch {
    return [] as string[];
  }
}

function persistKnownPasskeyCredentialDigests(digests: string[]) {
  if (typeof window === "undefined") {
    return;
  }

  const normalizedDigests = normalizePasskeyCredentialIds(digests);
  if (normalizedDigests.length === 0) {
    window.localStorage.removeItem(PASSKEY_CREDENTIAL_IDS_STORAGE_KEY);
    return;
  }

  window.localStorage.setItem(
    PASSKEY_CREDENTIAL_IDS_STORAGE_KEY,
    JSON.stringify(normalizedDigests),
  );
}

async function digestPasskeyCredentialId(
  credentialId: string,
): Promise<string | null> {
  if (
    typeof window === "undefined" ||
    !window.isSecureContext ||
    typeof window.crypto === "undefined" ||
    !window.crypto.subtle
  ) {
    return null;
  }

  const normalizedCredentialId = credentialId.trim();
  if (!normalizedCredentialId) {
    return null;
  }

  const bytes = new TextEncoder().encode(normalizedCredentialId);
  const digest = await window.crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest), (value) =>
    value.toString(16).padStart(2, "0"),
  ).join("");
}

async function rememberKnownPasskeyCredentialId(credentialId: unknown) {
  if (typeof credentialId !== "string") {
    return;
  }

  const digest = await digestPasskeyCredentialId(credentialId);
  if (!digest) {
    return;
  }

  const knownDigests = readKnownPasskeyCredentialDigests();
  if (knownDigests.includes(digest)) {
    return;
  }

  persistKnownPasskeyCredentialDigests([...knownDigests, digest]);
}

async function hasKnownPasskeyCredential(credentialIds: unknown) {
  const knownDigests = new Set(readKnownPasskeyCredentialDigests());
  if (knownDigests.size === 0) {
    return false;
  }

  const normalizedCredentialIds = normalizePasskeyCredentialIds(credentialIds);
  for (const credentialId of normalizedCredentialIds) {
    const digest = await digestPasskeyCredentialId(credentialId);
    if (digest && knownDigests.has(digest)) {
      return true;
    }
  }

  return false;
}

function persistPasskeyBindPromptPreference() {
  if (typeof window === "undefined") {
    return;
  }

  if (skipPasskeyBindPrompt.value) {
    window.localStorage.setItem(PASSKEY_BIND_PROMPT_STORAGE_KEY, "1");
    return;
  }

  window.localStorage.removeItem(PASSKEY_BIND_PROMPT_STORAGE_KEY);
}

function handlePasskeyBindDialogOpenChange(open: boolean) {
  if (open) {
    showPasskeyBindDialog.value = true;
    return;
  }

  if (!showPasskeyBindDialog.value) {
    return;
  }

  skipPasskeyBind();
}

async function handleLogin() {
  if (
    isLoading.value ||
    isLoginCoolingDown.value ||
    showPasskeyBindDialog.value ||
    pendingRunType.value !== null ||
    isBindingPasskey.value
  ) {
    return;
  }
  if (token.value.length !== 6) {
    errorMessage.value = t("auth.invalidOtpLength");
    showErrorDialog.value = true;
    return;
  }
  if (!isCaptchaVerified.value || !captchaSubmission.value) {
    errorMessage.value = t("auth.captchaFirst");
    showErrorDialog.value = true;
    return;
  }

  const now = Date.now();
  if (now - lastLoginAttemptAt < 400) {
    return;
  }
  lastLoginAttemptAt = now;

  isLoading.value = true;
  errorMessage.value = "";

  try {
    const res = await apiClient.post("/login", {
      token: token.value,
      captcha: captchaSubmission.value,
      rememberMe: rememberMe.value,
      redirect_uri: redirectUri || undefined,
    });

    if (res.data.success) {
      const runType = (res.data.data?.run_type ?? 3) as 0 | 1 | 3;
      const redirectTo =
        typeof res.data.data?.redirect_to === "string"
          ? res.data.data.redirect_to
          : null;
      const passkey = isPasskeySupported.value ? res.data.data?.passkey : null;
      if (
        isPasskeySupported.value &&
        passkey?.can_bind &&
        passkey?.bind_token
      ) {
        if (await hasKnownPasskeyCredential(passkey?.credential_ids)) {
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
    } else {
      const retryAfter = startLoginCooldown(extractRetryAfterSeconds(res.data));
      errorMessage.value = resolveRetryAfterMessage(
        res.data.message || t("auth.loginFailed"),
        retryAfter,
      );
      showErrorDialog.value = true;
      resetLoginState();
    }
  } catch (e: any) {
    console.error("Login error:", e);
    const retryAfter = startLoginCooldown(extractRetryAfterSeconds(e));
    errorMessage.value = resolveRetryAfterMessage(
      e?.response?.data?.message || t("auth.loginFailed"),
      retryAfter,
    );
    showErrorDialog.value = true;
    resetLoginState();
  } finally {
    isLoading.value = false;
  }
}

function completeLogin(runType: 0 | 1 | 3, redirectTo?: string | null) {
  pendingRunType.value = null;
  pendingRedirectTo.value = null;
  markPendingLogoutDelay();
  if (redirectTo) {
    window.location.replace(redirectTo);
    return;
  }
  if (runType === 0) {
    router.replace("/");
  } else {
    window.location.replace("/");
  }
}

async function handlePasskeyLogin() {
  if (
    !isPasskeySupported.value ||
    !isPasskeyAvailable.value ||
    isLoginCoolingDown.value ||
    isPasskeyLoading.value
  ) {
    return;
  }
  isPasskeyLoading.value = true;
  errorMessage.value = "";
  try {
    const optionsRes = await apiClient.post("/passkey/auth/options");
    const requestOptions = normalizeRequestOptions(optionsRes.data.data);
    const credential = await navigator.credentials.get({
      publicKey: requestOptions,
    });
    if (!credential) {
      throw new Error(t("auth.passkeyNoResponse"));
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
    const retryAfter = startLoginCooldown(
      extractRetryAfterSeconds(verifyRes.data),
    );
    throw new Error(
      resolveRetryAfterMessage(
        verifyRes.data.message || t("auth.passkeyVerifyFailed"),
        retryAfter,
      ),
    );
  } catch (e: any) {
    const retryAfter = startLoginCooldown(extractRetryAfterSeconds(e));
    errorMessage.value = resolveRetryAfterMessage(
      e?.response?.data?.message ||
        e?.message ||
        t("auth.passkeyLoginFailed"),
      retryAfter,
    );
    showErrorDialog.value = true;
  } finally {
    isPasskeyLoading.value = false;
  }
}

async function handleOidcLogin(providerId: string) {
  if (isOidcLoading.value || isLoginCoolingDown.value) return;
  isOidcLoading.value = true;
  activeOidcProviderId.value = providerId;
  errorMessage.value = "";
  try {
    const res = await apiClient.post("/oidc/start", {
      provider_id: providerId,
      mode: "login",
      rememberMe: rememberMe.value,
      redirect_uri: redirectUri || undefined,
    });
    const authorizationUrl = res.data?.data?.authorization_url;
    if (!authorizationUrl) {
      throw new Error(res.data?.message || t("auth.oidcStartFailed"));
    }
    window.location.assign(authorizationUrl);
  } catch (e: any) {
    errorMessage.value =
      e?.response?.data?.message || e?.message || t("auth.oidcLoginFailed");
    showErrorDialog.value = true;
    isOidcLoading.value = false;
    activeOidcProviderId.value = "";
  }
}

async function handlePasskeyBind() {
  if (isBindingPasskey.value) {
    return;
  }
  if (!passkeyBindToken.value) {
    passkeyBindError.value = t("auth.passkeyBindInvalid");
    return;
  }
  isBindingPasskey.value = true;
  passkeyBindError.value = "";
  try {
    const optionsRes = await apiClient.post("/passkey/register/options", {
      token: passkeyBindToken.value,
    });
    const creationOptions = normalizeCreationOptions(optionsRes.data.data);
    const credential = await navigator.credentials.create({
      publicKey: creationOptions,
    });
    if (!credential) {
      throw new Error(t("auth.passkeyNoResponse"));
    }
    const deviceName =
      (navigator as any).userAgentData?.platform ||
      navigator.platform ||
      "Unknown Device";
    const payload = serializeCredential(credential as PublicKeyCredential);
    const verifyRes = await apiClient.post("/passkey/register/verify", {
      token: passkeyBindToken.value,
      deviceName,
      credential: payload,
    });
    if (verifyRes.data.success) {
      await rememberKnownPasskeyCredentialId(payload.id);
      isPasskeyAvailable.value = true;
      showPasskeyBindDialog.value = false;
      passkeyBindToken.value = "";
      skipPasskeyBindPrompt.value = false;
      if (pendingRunType.value !== null) {
        completeLogin(pendingRunType.value, pendingRedirectTo.value);
      }
      return;
    }
    throw new Error(verifyRes.data.message || t("auth.passkeyBindFailed"));
  } catch (e: any) {
    passkeyBindError.value =
      e?.response?.data?.message || e?.message || t("auth.passkeyBindFailed");
  } finally {
    isBindingPasskey.value = false;
  }
}

function skipPasskeyBind() {
  persistPasskeyBindPromptPreference();
  showPasskeyBindDialog.value = false;
  passkeyBindToken.value = "";
  passkeyBindError.value = "";
  skipPasskeyBindPrompt.value = false;
  if (pendingRunType.value !== null) {
    completeLogin(pendingRunType.value, pendingRedirectTo.value);
  }
}

function resetLoginState() {
  token.value = "";
  handleCaptchaReset();
  if (
    activeCaptchaProvider.value === "pow" &&
    canUseNativePow.value &&
    powWidgetRef.value
  ) {
    powWidgetRef.value.reset();
  }
  if (activeCaptchaProvider.value === "turnstile" && turnstileWidgetRef.value) {
    turnstileWidgetRef.value.reset();
  }
}
</script>
