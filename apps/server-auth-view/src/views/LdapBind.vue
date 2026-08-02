<template>
  <AuthShell>
    <AuthCard
      :title="t('auth.ldapBind.title')"
      :description="t('auth.ldapBind.description')"
      content-class="space-y-4"
    >
      <div
        v-if="isLoading"
        class="py-8 text-center text-sm text-muted-foreground"
        role="status"
      >
        {{ t("auth.ldapBind.checkingInvite") }}
      </div>
      <div
        v-else-if="errorMessage && !invite"
        class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        role="alert"
      >
        {{ errorMessage }}
      </div>
      <form v-else-if="invite" class="space-y-4" @submit.prevent="bindIdentity">
        <div class="rounded-lg border bg-muted/40 px-3 py-2 text-sm">
          <div class="text-muted-foreground">
            {{ t("auth.ldapBind.bindTo") }}
          </div>
          <div class="font-medium">{{ invite.totp.comment || "TOTP" }}</div>
          <div class="mt-1 text-xs text-muted-foreground">
            {{ invite.provider.name }}
          </div>
        </div>

        <div v-if="!isCaptchaVerified" class="space-y-3">
          <Button
            v-if="activeCaptchaProvider === 'pow' && isCaptchaProviderAvailable"
            type="button"
            class="w-full"
            :disabled="isPowFallbackLoading || isLoginCoolingDown"
            @click="handlePowFallbackVerify"
          >
            {{
              isPowFallbackLoading ? t("auth.verifying") : t("auth.notRobot")
            }}
          </Button>
          <TurnstileWidget
            v-else-if="
              activeCaptchaProvider === 'turnstile' &&
              isCaptchaProviderAvailable &&
              hasTurnstileSiteKey
            "
            ref="turnstileWidgetRef"
            :site-key="captchaConfig?.turnstile.site_key || ''"
            :disabled="isSubmitting || isLoginCoolingDown"
            @verified="handleTurnstileVerified"
            @expired="resetCaptcha"
            @reset="resetCaptcha"
            @error="handleTurnstileError"
          />
          <div
            v-else-if="!isCaptchaConfigLoading"
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
          >
            {{ captchaUnavailableReason }}
          </div>
        </div>

        <template v-else>
          <div class="space-y-2">
            <Label for="ldap-bind-username">{{ t("auth.ldapUsername") }}</Label>
            <Input
              id="ldap-bind-username"
              v-model="username"
              autocomplete="username"
              :disabled="isSubmitting || isLoginCoolingDown"
            />
          </div>
          <div class="space-y-2">
            <Label for="ldap-bind-password">{{ t("auth.ldapPassword") }}</Label>
            <Input
              id="ldap-bind-password"
              v-model="password"
              type="password"
              autocomplete="current-password"
              :disabled="isSubmitting || isLoginCoolingDown"
            />
          </div>
          <div
            class="flex items-center gap-3 rounded-lg border bg-muted/40 px-3 py-2"
          >
            <Checkbox
              id="ldap-bind-remember"
              v-model="rememberMe"
              :disabled="isSubmitting || isLoginCoolingDown"
            />
            <label for="ldap-bind-remember" class="cursor-pointer text-sm">
              {{ t("auth.rememberMe") }}
            </label>
          </div>
          <div
            v-if="errorMessage"
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
            role="alert"
          >
            {{ errorMessage }}
          </div>
          <Button
            type="submit"
            class="w-full"
            :disabled="isSubmitting || isLoginCoolingDown"
          >
            {{
              isSubmitting
                ? t("auth.verifying")
                : isLoginCoolingDown
                  ? t("auth.retryAfterSeconds", {
                      seconds: loginCooldownSeconds,
                    })
                  : t("auth.ldapBind.bindNow")
            }}
          </Button>
        </template>
      </form>
    </AuthCard>
  </AuthShell>
</template>

<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { LocaleConfig } from "@fn-knock/i18n/core";
import type { AppearanceConfig } from "@frontend-core/appearance";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import AuthCard from "@/components/AuthCard.vue";
import AuthShell from "@/components/AuthShell.vue";
import TurnstileWidget from "@/components/captcha/TurnstileWidget.vue";
import { useAuthSystemConfig } from "@/composables/useAuthSystemConfig";
import { useLoginCaptcha } from "@/composables/useLoginCaptcha";
import { useLoginCooldown } from "@/composables/useLoginCooldown";
import { apiClient, CaptchaAPI } from "@/lib/api";

type InviteDetails = {
  locale: LocaleConfig;
  appearance: AppearanceConfig;
  totp: { id: string; comment: string };
  provider: { id: string; name: string; protocol: "ldap" };
  expires_at: string;
};

const params =
  typeof window !== "undefined"
    ? new URLSearchParams(window.location.search)
    : new URLSearchParams();
const token = params.get("token") || "";
const redirectUri = params.get("redirect_uri") || "";
const invite = ref<InviteDetails | null>(null);
const username = ref("");
const password = ref("");
const rememberMe = ref(false);
const errorMessage = ref("");
const isLoading = ref(true);
const isSubmitting = ref(false);
const i18n = useI18n();
const { t } = i18n;
const { applyAuthSystemConfig } = useAuthSystemConfig(i18n);

const {
  activeCaptchaProvider,
  captchaConfig,
  captchaSubmission,
  captchaUnavailableReason,
  handlePowFallbackVerify,
  handleTurnstileError,
  handleTurnstileVerified,
  hasTurnstileSiteKey,
  isCaptchaConfigLoading,
  isCaptchaProviderAvailable,
  isCaptchaVerified,
  isPowFallbackLoading,
  resetCaptcha,
  resetCaptchaWidgets,
  turnstileWidgetRef,
} = useLoginCaptcha({
  canUseNativePow: false,
  translate: (key) => t(key),
  onError: (message) => {
    errorMessage.value = message;
  },
});
void turnstileWidgetRef;
const {
  isCoolingDown: isLoginCoolingDown,
  remainingSeconds: loginCooldownSeconds,
  resolveMessage: resolveLoginCooldownMessage,
} = useLoginCooldown({
  formatRetrySuffix: (seconds) => t("auth.retrySuffix", { seconds }),
});

onMounted(loadInvite);

async function loadInvite() {
  try {
    if (!token) throw new Error(t("auth.ldapBind.missingToken"));
    const [inviteResponse, captcha] = await Promise.all([
      apiClient.get("/ldap/invite", { params: { token } }),
      CaptchaAPI.getConfig(),
    ]);
    invite.value = inviteResponse.data.data;
    captchaConfig.value = captcha;
    await applyAuthSystemConfig(invite.value);
  } catch (error: any) {
    await applyAuthSystemConfig(error?.response?.data?.data);
    errorMessage.value =
      error?.response?.data?.message ||
      error?.message ||
      t("auth.ldapBind.inviteExpired");
  } finally {
    isCaptchaConfigLoading.value = false;
    isLoading.value = false;
  }
}

async function bindIdentity() {
  if (isSubmitting.value || isLoginCoolingDown.value) return;
  if (!username.value.trim() || !password.value) {
    errorMessage.value = t("auth.usernamePasswordRequired");
    return;
  }
  if (!captchaSubmission.value) {
    errorMessage.value = t("auth.captchaFirst");
    return;
  }
  isSubmitting.value = true;
  errorMessage.value = "";
  try {
    const response = await apiClient.post("/ldap/bind", {
      token,
      username: username.value.trim(),
      password: password.value,
      captcha: captchaSubmission.value,
      rememberMe: rememberMe.value,
      redirect_uri: redirectUri || undefined,
    });
    const redirectTo = response.data?.data?.redirect_to;
    window.location.assign(
      typeof redirectTo === "string" && redirectTo ? redirectTo : "/",
    );
  } catch (error: any) {
    password.value = "";
    errorMessage.value = resolveLoginCooldownMessage(
      error?.response?.data?.message ||
        error?.message ||
        t("auth.ldapBind.bindFailed"),
      error,
    );
    resetCaptchaWidgets();
  } finally {
    isSubmitting.value = false;
  }
}
</script>
