<template>
  <AuthShell>
    <AuthCard v-if="isCheckingAuth" content-class="flex flex-col gap-4">
      <template #header>
        <div
          class="grid auto-rows-min grid-rows-[auto_auto] items-start gap-1.5 px-6"
        >
          <Skeleton class="h-8 w-44 mx-auto" />
          <Skeleton class="h-4 w-48 mx-auto mt-2" />
        </div>
      </template>
      <Skeleton class="h-4 w-full" />
      <Skeleton class="h-9 w-full rounded-md" />
    </AuthCard>

    <AuthCard
      v-else
      :title="statusTitle"
      :description="statusDescription"
      content-class="flex flex-col gap-4"
    >
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
          {{ t("auth.home.enablePasskey") }}
        </Button>
        <p class="text-xs text-center text-muted-foreground">
          {{ t("auth.home.passkeySupportedUnbound") }}
        </p>
      </div>
      <p v-if="passkeyError" class="text-xs text-center text-destructive">
        {{ passkeyError }}
      </p>
      <p
        v-if="!canShowLogoutButton"
        class="text-xs text-center text-muted-foreground"
      >
        {{
          t("auth.home.logoutDelay", {
            seconds: logoutDelayRemainingSeconds,
          })
        }}
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
        {{ t("auth.home.logout") }}
      </Button>
    </AuthCard>

    <template #footer>
      <AuthFooter
        :client-ip="clientIp"
        :ip-location="ipLocation"
        :ip-location-status="ipLocationStatus"
      />
    </template>
  </AuthShell>

  <Dialog
    :open="showLogoutConfirmDialog"
    @update:open="showLogoutConfirmDialog = $event"
  >
    <DialogContent :show-close-button="false">
      <DialogHeader>
        <DialogTitle>{{ t("auth.home.logoutConfirmTitle") }}</DialogTitle>
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
          {{ t("common.cancel") }}
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
          {{ t("auth.home.confirmLogout") }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
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
import type { AuthGrantType } from "@frontend-core/auth/types";
import { apiClient, AuthAPI } from "@/lib/api";
import { useClientIpLocation } from "@/lib/client-ip-location";
import {
  consumePendingLogoutDelay,
  POST_LOGIN_LOGOUT_DELAY_MS,
} from "@/lib/post-login";
import AuthFooter from "@/components/AuthFooter.vue";
import AuthCard from "@/components/AuthCard.vue";
import AuthShell from "@/components/AuthShell.vue";
import { useAuthBrowserCapabilities } from "@/composables/useAuthBrowserCapabilities";
import { useAuthSystemConfig } from "@/composables/useAuthSystemConfig";
import { usePasskeyRegistration } from "@/composables/usePasskeyRegistration";

const router = useRouter();
const i18n = useI18n();
const { t } = i18n;
const isLoading = ref(false);
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
const { isPasskeySupported, refreshBrowserCapabilities } =
  useAuthBrowserCapabilities();
const { applyAuthSystemConfig } = useAuthSystemConfig(i18n);
const { registerPasskeyCredential } = usePasskeyRegistration();

const resolveGrantKey = (grantType?: AuthGrantType) => {
  switch (grantType) {
    case "browser_session":
      return "browserSession";
    case "session_migration":
      return "sessionMigration";
    case "fnos_fingerprint_session":
      return "fnosFingerprintSession";
    case "manual_whitelist":
      return "manualWhitelist";
    case "local_exempt":
      return "localExempt";
    case "fnos_share":
      return "fnosShare";
    case "login_ip_grant":
    default:
      return "loginIpGrant";
  }
};

const grantKey = computed(() => resolveGrantKey(authGrantType.value));

const statusTitle = computed(() =>
  t(`auth.home.statusTitles.${grantKey.value}`),
);

const statusDescription = computed(() =>
  t(`auth.home.statusDescriptions.${grantKey.value}`),
);

const logoutHint = computed(() => t(`auth.home.logoutHints.${grantKey.value}`));

const logoutDialogKey = computed(() =>
  authGrantType.value ? grantKey.value : "default",
);

const logoutDialogDescription = computed(() =>
  t(`auth.home.logoutDialogDescriptions.${logoutDialogKey.value}`),
);

let logoutDelayTimer: ReturnType<typeof window.setTimeout> | null = null;
let logoutDelayCountdownTimer: ReturnType<typeof window.setInterval> | null =
  null;

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
    await applyAuthSystemConfig(session);
    startLocationPolling(session.client);
    isPasskeyAvailable.value = !!session.passkey.available;
    authGrantType.value = session.auth.grant_type;
    return true;
  } catch (e: any) {
    console.error("Auth session request failed:", e);
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
  refreshBrowserCapabilities();
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
  if (
    isPasskeyBinding.value ||
    !isPasskeySupported.value ||
    isPasskeyAvailable.value
  ) {
    return;
  }
  isPasskeyBinding.value = true;
  passkeyError.value = "";
  try {
    const tokenRes = await apiClient.post("/passkey/bind-token");
    const bindToken = tokenRes.data?.data?.token;
    if (!bindToken) {
      throw new Error(t("auth.home.passkeyTokenMissing"));
    }
    await registerPasskeyCredential(bindToken, {
      bindFailed: t("auth.passkeyBindFailed"),
      noResponse: t("auth.passkeyNoResponse"),
    });
    isPasskeyAvailable.value = true;
  } catch (e: any) {
    passkeyError.value =
      e?.response?.data?.message || e?.message || t("auth.passkeyBindFailed");
  } finally {
    isPasskeyBinding.value = false;
  }
}
</script>
