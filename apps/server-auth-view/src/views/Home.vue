<template>
  <div class="min-h-screen flex flex-col bg-muted/40 p-4">
    <div class="flex flex-1 items-center justify-center">
      <Card v-if="isCheckingAuth" class="w-full max-w-sm">
        <CardHeader>
          <Skeleton class="h-8 w-44 mx-auto" />
          <Skeleton class="h-4 w-48 mx-auto mt-2" />
        </CardHeader>
        <CardContent class="flex flex-col gap-4">
          <Skeleton class="h-4 w-full" />
          <Skeleton class="h-9 w-full rounded-md" />
        </CardContent>
      </Card>

      <Card v-else class="w-full max-w-sm">
        <CardHeader>
          <CardTitle class="text-2xl text-center">{{ statusTitle }}</CardTitle>
          <CardDescription class="text-center">
            {{ statusDescription }}
          </CardDescription>
        </CardHeader>

        <CardContent class="flex flex-col gap-4">
          <p class="text-sm text-center text-muted-foreground">
            {{ logoutHint }}
          </p>
          <div
            v-if="isPasskeySupported && !isPasskeyAvailable"
            class="flex flex-col gap-2"
          >
            <Button
              class="w-full"
              :disabled="isPasskeyBinding"
              @click="handlePasskeyBind"
            >
              <span
                v-if="isPasskeyBinding"
                class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
              ></span>
              开启 Passkey 一键登录
            </Button>
            <p class="text-xs text-center text-muted-foreground">
              当前浏览器支持 Passkey，但尚未绑定
            </p>
          </div>
          <p v-if="passkeyError" class="text-xs text-center text-destructive">
            {{ passkeyError }}
          </p>
          <p
            v-if="!canShowLogoutButton"
            class="text-xs text-center text-muted-foreground"
          >
            退出登录按钮将在 {{ logoutDelayRemainingSeconds }} 秒后显示
          </p>
          <Button
            v-else
            variant="destructive"
            @click="openLogoutConfirm"
            class="w-full"
            :disabled="isLoading"
          >
            <span
              v-if="isLoading"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            退出登录
          </Button>
        </CardContent>
      </Card>
    </div>

    <AuthFooter
      :client-ip="clientIp"
      :ip-location="ipLocation"
      :ip-location-status="ipLocationStatus"
    />
  </div>

  <Dialog
    :open="showLogoutConfirmDialog"
    @update:open="showLogoutConfirmDialog = $event"
  >
    <DialogContent :show-close-button="false">
      <DialogHeader>
        <DialogTitle>确认退出登录</DialogTitle>
        <DialogDescription>
          {{ logoutDialogDescription }}
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="gap-2">
        <Button
          variant="outline"
          @click="showLogoutConfirmDialog = false"
          :disabled="isLoading"
        >
          取消
        </Button>
        <Button
          variant="destructive"
          @click="handleLogout"
          :disabled="isLoading"
        >
          <span
            v-if="isLoading"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          确认退出
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import {
  Card,
  CardHeader,
  CardTitle,
  CardDescription,
  CardContent,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
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
  serializeCredential,
} from "@frontend-core/passkey/utils";
import type { AuthGrantType } from "@frontend-core/auth/types";
import { apiClient, AuthAPI } from "@/lib/api";
import { useClientIpLocation } from "@/lib/client-ip-location";
import {
  consumePendingLogoutDelay,
  POST_LOGIN_LOGOUT_DELAY_MS,
} from "@/lib/post-login";
import AuthFooter from "@/components/AuthFooter.vue";

const router = useRouter();
const isLoading = ref(false);
const isPasskeySupported = ref(false);
const isPasskeyAvailable = ref(false);
const isPasskeyBinding = ref(false);
const passkeyError = ref("");
const isCheckingAuth = ref(true);
const { clientIp, ipLocation, ipLocationStatus, startLocationPolling } =
  useClientIpLocation();
const canShowLogoutButton = ref(true);
const logoutDelayRemainingSeconds = ref(0);
const showLogoutConfirmDialog = ref(false);
const authGrantType = ref<AuthGrantType | undefined>(undefined);

const statusTitle = computed(() => {
  switch (authGrantType.value) {
    case "browser_session":
      return "当前浏览器会话已验证";
    case "session_migration":
      return "浏览器会话已恢复";
    case "fnos_fingerprint_session":
      return "设备指纹会话已恢复";
    case "manual_whitelist":
      return "白名单访问已放行";
    case "local_exempt":
      return "当前网络已放行";
    case "fnos_share":
      return "分享访问已授权";
    case "login_ip_grant":
    default:
      return "安全验证已通过";
  }
});

const statusDescription = computed(() => {
  switch (authGrantType.value) {
    case "browser_session":
      return "当前浏览器会话已被允许访问";
    case "session_migration":
      return "当前浏览器会话已随网络切换恢复访问";
    case "fnos_fingerprint_session":
      return "当前访问已由飞牛设备指纹会话恢复";
    case "manual_whitelist":
      return "当前 IP 已在管理员白名单中";
    case "local_exempt":
      return "当前网络地址属于免白名单范围";
    case "fnos_share":
      return "当前访问由飞牛分享链路授权";
    case "login_ip_grant":
    default:
      return "您的 IP 已被授权访问";
  }
});

const logoutHint = computed(() => {
  switch (authGrantType.value) {
    case "browser_session":
      return "如果不再需要访问，请点击下方按钮退出。退出后当前浏览器需要重新验证才能再次进入。";
    case "session_migration":
      return "如果不再需要访问，请点击下方按钮退出。退出后当前浏览器需要重新验证，并会撤销本次会话迁移关联的授权。";
    case "fnos_fingerprint_session":
      return "如果不再需要访问，请点击下方按钮退出。退出后当前恢复的设备指纹会话会结束，并撤销关联授权。";
    case "login_ip_grant":
      return "如果不再需要访问，请点击下方按钮退出。退出后当前浏览器会话会结束，登录时授予的当前 IP 访问权限也会一并撤销。";
    case "manual_whitelist":
      return "如果不再需要访问，请点击下方按钮退出。退出只会结束当前浏览器会话，管理员白名单不会被移除。";
    case "local_exempt":
      return "如果不再需要访问，请点击下方按钮退出。退出只会结束当前浏览器会话，免白名单网络访问范围不会改变。";
    case "fnos_share":
      return "如果不再需要访问，请点击下方按钮退出。退出后当前分享访问会话会结束，需要重新进入分享链路。";
    default:
      return "如果不再需要访问，请点击下方按钮退出并撤销您的授权。";
  }
});

const logoutDialogDescription = computed(() => {
  switch (authGrantType.value) {
    case "browser_session":
      return "退出后将结束当前浏览器会话，需要重新验证后才能再次进入。";
    case "session_migration":
      return "退出后将结束当前浏览器会话，并撤销本次会话迁移关联的授权。";
    case "fnos_fingerprint_session":
      return "退出后将结束当前恢复的设备指纹会话，并撤销关联授权。";
    case "login_ip_grant":
      return "退出后将结束当前浏览器会话，并撤销这次登录授予的当前 IP 访问权限。";
    case "manual_whitelist":
      return "退出后只会结束当前浏览器会话，管理员配置的白名单不会被移除。";
    case "local_exempt":
      return "退出后只会结束当前浏览器会话，当前网络的免白名单属性不会改变。";
    case "fnos_share":
      return "退出后将结束当前分享访问会话，如需再次访问请重新进入分享链路。";
    default:
      return "退出后将撤销当前访问授权，需要重新验证后才能再次进入。";
  }
});

let logoutDelayTimer: ReturnType<typeof window.setTimeout> | null = null;
let logoutDelayCountdownTimer: ReturnType<typeof window.setInterval> | null =
  null;

function initPasskeySupport() {
  isPasskeySupported.value =
    typeof window !== "undefined" && !!window.PublicKeyCredential;
}

function clearLogoutDelayTimers() {
  if (logoutDelayTimer) {
    window.clearTimeout(logoutDelayTimer);
    logoutDelayTimer = null;
  }
  if (logoutDelayCountdownTimer) {
    window.clearInterval(logoutDelayCountdownTimer);
    logoutDelayCountdownTimer = null;
  }
}

function initLogoutAvailability() {
  if (!consumePendingLogoutDelay()) {
    canShowLogoutButton.value = true;
    logoutDelayRemainingSeconds.value = 0;
    return;
  }

  canShowLogoutButton.value = false;
  logoutDelayRemainingSeconds.value = Math.ceil(
    POST_LOGIN_LOGOUT_DELAY_MS / 1000,
  );

  logoutDelayCountdownTimer = window.setInterval(() => {
    if (logoutDelayRemainingSeconds.value <= 1) {
      logoutDelayRemainingSeconds.value = 0;
      if (logoutDelayCountdownTimer) {
        window.clearInterval(logoutDelayCountdownTimer);
        logoutDelayCountdownTimer = null;
      }
      return;
    }

    logoutDelayRemainingSeconds.value -= 1;
  }, 1000);

  logoutDelayTimer = window.setTimeout(() => {
    canShowLogoutButton.value = true;
    logoutDelayRemainingSeconds.value = 0;
    clearLogoutDelayTimers();
  }, POST_LOGIN_LOGOUT_DELAY_MS);
}

async function loadSession() {
  try {
    const session = await AuthAPI.getSession();
    startLocationPolling(session.client);
    isPasskeyAvailable.value = !!session.passkey.available;
    authGrantType.value = session.auth.grant_type;
    return true;
  } catch (e: any) {
    console.error("身份验证请求异常:", e);
    const query = Object.fromEntries(
      new URLSearchParams(
        typeof window !== "undefined" ? window.location.search : "",
      ).entries(),
    );
    await router.replace({ path: "/login", query });
    return false;
  } finally {
    isCheckingAuth.value = false;
  }
}
onMounted(async () => {
  initPasskeySupport();
  const isAuthenticated = await loadSession();
  if (!isAuthenticated) {
    return;
  }

  initLogoutAvailability();
});

onBeforeUnmount(() => {
  clearLogoutDelayTimers();
});

function openLogoutConfirm() {
  showLogoutConfirmDialog.value = true;
}

async function handleLogout() {
  isLoading.value = true;
  try {
    showLogoutConfirmDialog.value = false;
    await apiClient.get("/logout");
    await router.replace({
      path: "/login",
      query: { logged_out: "1" },
    });
  } catch (e) {
    console.error("Logout failed:", e);
  } finally {
    isLoading.value = false;
  }
}

async function handlePasskeyBind() {
  if (!isPasskeySupported.value || isPasskeyAvailable.value) return;
  isPasskeyBinding.value = true;
  passkeyError.value = "";
  try {
    const tokenRes = await apiClient.post("/passkey/bind-token");
    const bindToken = tokenRes.data?.data?.token;
    if (!bindToken) {
      throw new Error("无法获取绑定凭证");
    }
    const optionsRes = await apiClient.post("/passkey/register/options", {
      token: bindToken,
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
      token: bindToken,
      deviceName,
      credential: payload,
    });
    if (verifyRes.data.success) {
      isPasskeyAvailable.value = true;
      return;
    }
    throw new Error(verifyRes.data.message || "Passkey 绑定失败");
  } catch (e: any) {
    passkeyError.value =
      e?.response?.data?.message || e?.message || "Passkey 绑定失败";
  } finally {
    isPasskeyBinding.value = false;
  }
}
</script>
