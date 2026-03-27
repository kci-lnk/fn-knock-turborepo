<template>
  <div class="min-h-screen flex flex-col bg-muted/40 p-4">
    <div class="flex flex-1 items-center justify-center">
      <Card class="w-full max-w-sm">
        <CardHeader>
          <CardTitle class="text-2xl text-center">安全验证</CardTitle>
          <CardDescription class="text-center" v-if="!isCaptchaVerified">
            请先完成下方的人机验证
          </CardDescription>
          <CardDescription class="text-center" v-else>
            请输入您的六位数动态密码完成登录
          </CardDescription>
          <div
            v-if="logoutNotice"
            class="mt-3 rounded-lg border border-zinc-200 bg-zinc-50 px-3 py-2 text-sm text-zinc-700"
          >
            {{ logoutNotice }}
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
                  JSON.stringify({
                    label: '我不是机器人',
                    verified: '验证通过',
                    verifying: '正在验证...',
                    wait: '请稍候...',
                    error: '验证错误',
                  })
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
                {{ isPowFallbackLoading ? "正在验证..." : "我不是机器人" }}
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
                当前 Turnstile 未完成配置，请联系管理员填写 site key。
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
                :disabled="isPasskeyLoading"
                @click="handlePasskeyLogin"
              >
                <span
                  v-if="isPasskeyLoading"
                  class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                ></span>
                Passkey 一键登录
              </Button>
            </div>

            <div class="w-full flex justify-center" v-if="isCaptchaVerified">
              <InputOTP
                inputmode="numeric"
                :maxlength="6"
                v-model="token"
                @complete="handleOtpComplete"
                :disabled="isLoading"
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

            <div class="w-full flex justify-center" v-if="isCaptchaVerified">
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
                  记住我
                </label>
              </div>
            </div>

            <Dialog
              :open="showErrorDialog"
              @update:open="showErrorDialog = $event"
            >
              <DialogContent :show-close-button="false">
                <DialogHeader>
                  <DialogTitle>提示</DialogTitle>
                  <DialogDescription>
                    {{ errorMessage }}
                  </DialogDescription>
                </DialogHeader>
                <DialogFooter>
                  <Button @click="showErrorDialog = false">确定</Button>
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
                  <DialogTitle>开启 Passkey 一键登录</DialogTitle>
                  <DialogDescription>
                    是否在当前设备上绑定 Passkey？绑定后可直接一键登录。
                  </DialogDescription>
                </DialogHeader>
                <div v-if="passkeyBindError" class="text-sm text-destructive">
                  {{ passkeyBindError }}
                </div>
                <div class="flex items-center space-x-3 rounded-lg border bg-muted/40 px-3 py-2">
                  <Checkbox
                    id="skipPasskeyBindPrompt"
                    v-model="skipPasskeyBindPrompt"
                    :disabled="isBindingPasskey"
                  />
                  <label
                    for="skipPasskeyBindPrompt"
                    class="cursor-pointer select-none text-sm text-muted-foreground"
                  >
                    不再提醒
                  </label>
                </div>
                <DialogFooter class="gap-2">
                  <Button variant="outline" @click="skipPasskeyBind"
                    >稍后再说</Button
                  >
                  <Button
                    :disabled="isBindingPasskey"
                    @click="handlePasskeyBind"
                  >
                    <span
                      v-if="isBindingPasskey"
                      class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
                    ></span>
                    立即开启
                  </Button>
                </DialogFooter>
              </DialogContent>
            </Dialog>
            <Button
              type="button"
              class="w-full"
              :disabled="isLoading"
              v-if="isCaptchaVerified"
              @click="handleLogin"
            >
              <span
                v-if="isLoading"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              立即验证
            </Button>
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
import { computed, ref, onMounted } from "vue";
import { useRouter } from "vue-router";
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
import type { AuthGrantType } from "@frontend-core/auth/types";
import type {
  CaptchaPublicSettings,
  CaptchaSubmission,
} from "@frontend-core/captcha/types";
import { apiClient, AuthAPI, buildAuthApiPath, CaptchaAPI } from "@/lib/api";
import { useClientIpLocation } from "@/lib/client-ip-location";
import {
  buildPowSubmission,
  normalizePowChallenge,
  solvePowChallenge,
} from "@/lib/captcha";
import { markPendingLogoutDelay } from "@/lib/post-login";
import AuthFooter from "@/components/AuthFooter.vue";
import TurnstileWidget from "@/components/captcha/TurnstileWidget.vue";

import "altcha";

const router = useRouter();

const token = ref("");
const rememberMe = ref(false);
const errorMessage = ref("");
const showErrorDialog = ref(false);
const isLoading = ref(false);
const isPasskeySupported = ref(false);
const isPasskeyAvailable = ref(false);
const isPasskeyLoading = ref(false);
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
const PASSKEY_BIND_PROMPT_STORAGE_KEY =
  "server-auth-view:passkey-bind-prompt-dismissed";

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
const activeCaptchaProvider = computed(
  () => captchaConfig.value?.provider ?? null,
);
const isCaptchaProviderAvailable = computed(
  () => captchaConfig.value?.available ?? false,
);
const captchaUnavailableReason = computed(
  () =>
    captchaConfig.value?.unavailable_reason ||
    "验证码配置加载失败，请刷新页面后重试。",
);
const hasTurnstileSiteKey = computed(
  () => !!captchaConfig.value?.turnstile.site_key.trim(),
);
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
      return "当前浏览器会话已退出，登录时授予的当前 IP 访问权限也已撤销。";
    case "manual_whitelist":
      return "当前浏览器会话已退出。管理员白名单仍然有效。";
    case "local_exempt":
      return "当前浏览器会话已退出。当前网络仍属于免白名单范围。";
    default:
      return "当前浏览器会话已退出，请重新验证。";
  }
});

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
    startLocationPolling(bootstrap.client);
    captchaConfig.value = bootstrap.captcha;
    isPasskeyAvailable.value = !!bootstrap.passkey.available;
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
      "验证码配置加载失败，请刷新页面后重试";
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
      e?.response?.data?.message || e?.message || "人机验证失败，请重试";
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

function handleOtpComplete() {
  void handleLogin();
}

function isPasskeyBindPromptDismissed() {
  if (typeof window === "undefined") {
    return false;
  }
  return window.localStorage.getItem(PASSKEY_BIND_PROMPT_STORAGE_KEY) === "1";
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
  if (isLoading.value) {
    return;
  }
  if (token.value.length !== 6) {
    errorMessage.value = "请输入完整的 6 位身份验证码";
    showErrorDialog.value = true;
    return;
  }
  if (!isCaptchaVerified.value || !captchaSubmission.value) {
    errorMessage.value = "请先完成人机验证";
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
      const runType = (res.data.data?.run_type ?? 1) as 0 | 1 | 3;
      const passkey = res.data.data?.passkey;
      const redirectTo =
        typeof res.data.data?.redirect_to === "string"
          ? res.data.data.redirect_to
          : null;
      if (
        isPasskeySupported.value &&
        passkey?.can_bind &&
        passkey?.bind_token
      ) {
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
      errorMessage.value = res.data.message || "验证失败，请重试";
      showErrorDialog.value = true;
      resetLoginState();
    }
  } catch (e: any) {
    console.error("Login error:", e);
    errorMessage.value = e?.response?.data?.message || "验证失败，请重试";
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
  if (!isPasskeySupported.value || !isPasskeyAvailable.value) return;
  isPasskeyLoading.value = true;
  errorMessage.value = "";
  try {
    const optionsRes = await apiClient.post("/passkey/auth/options");
    const requestOptions = normalizeRequestOptions(optionsRes.data.data);
    const credential = await navigator.credentials.get({
      publicKey: requestOptions,
    });
    if (!credential) {
      throw new Error("未获取到 Passkey 响应");
    }
    const payload = serializeCredential(credential as PublicKeyCredential);
    const verifyRes = await apiClient.post("/passkey/auth/verify", {
      credential: payload,
      rememberMe: rememberMe.value,
      redirect_uri: redirectUri || undefined,
    });
    if (verifyRes.data.success) {
      completeLogin(
        (verifyRes.data.data?.run_type ?? 1) as 0 | 1 | 3,
        typeof verifyRes.data.data?.redirect_to === "string"
          ? verifyRes.data.data.redirect_to
          : null,
      );
      return;
    }
    throw new Error(verifyRes.data.message || "Passkey 验证失败");
  } catch (e: any) {
    errorMessage.value =
      e?.response?.data?.message || e?.message || "Passkey 登录失败，请重试";
    showErrorDialog.value = true;
  } finally {
    isPasskeyLoading.value = false;
  }
}

async function handlePasskeyBind() {
  if (!passkeyBindToken.value) {
    passkeyBindError.value = "绑定凭证无效，请重新登录";
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
      throw new Error("未获取到 Passkey 响应");
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
      isPasskeyAvailable.value = true;
      showPasskeyBindDialog.value = false;
      passkeyBindToken.value = "";
      skipPasskeyBindPrompt.value = false;
      if (pendingRunType.value !== null) {
        completeLogin(pendingRunType.value, pendingRedirectTo.value);
      }
      return;
    }
    throw new Error(verifyRes.data.message || "Passkey 绑定失败");
  } catch (e: any) {
    passkeyBindError.value =
      e?.response?.data?.message || e?.message || "Passkey 绑定失败";
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
