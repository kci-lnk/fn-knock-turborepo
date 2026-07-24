<template>
  <AuthShell>
    <AuthCard
      :title="t('auth.oidcBind.title')"
      :description="description"
      content-class="space-y-4"
    >
      <div
        v-if="isLoading"
        class="py-8 text-center text-sm text-muted-foreground"
        role="status"
      >
        {{ t("auth.oidcBind.checkingInvite") }}
      </div>
      <div
        v-else-if="errorMessage"
        class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
        role="alert"
      >
        {{ errorMessage }}
      </div>
      <div v-else class="space-y-4">
        <div class="rounded-lg border bg-muted/40 px-3 py-2 text-sm">
          <div class="text-muted-foreground">
            {{ t("auth.oidcBind.bindTo") }}
          </div>
          <div class="font-medium">{{ invite?.totp.comment || "TOTP" }}</div>
        </div>
        <Button
          v-for="provider in invite?.providers || []"
          :key="provider.id"
          type="button"
          variant="outline"
          class="w-full"
          :disabled="isStarting"
          @click="startBind(provider.id)"
        >
          <span
            v-if="activeProviderId === provider.id && isStarting"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
          ></span>
          <QqIcon
            v-else-if="provider.type === 'fnknock_qq'"
            class="mr-2 size-4 text-[#1ebafc]"
            aria-hidden="true"
          />
          {{ t("auth.oidcBind.useProvider", { provider: provider.name }) }}
        </Button>
      </div>
    </AuthCard>
  </AuthShell>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { apiClient } from "@/lib/api";
import { useAuthSystemConfig } from "@/composables/useAuthSystemConfig";
import type { LocaleConfig } from "@fn-knock/i18n/core";
import type { AppearanceConfig } from "@frontend-core/appearance";
import AuthCard from "@/components/AuthCard.vue";
import AuthShell from "@/components/AuthShell.vue";
import QqIcon from "@/components/QqIcon.vue";

type InviteDetails = {
  locale: LocaleConfig;
  appearance: AppearanceConfig;
  totp: { id: string; comment: string };
  provider_id?: string;
  expires_at: string;
  providers: Array<{ id: string; type: string; name: string }>;
};

const params =
  typeof window !== "undefined"
    ? new URLSearchParams(window.location.search)
    : new URLSearchParams();
const token = params.get("token") || "";
const invite = ref<InviteDetails | null>(null);
const errorMessage = ref("");
const isLoading = ref(true);
const isStarting = ref(false);
const activeProviderId = ref("");
const i18n = useI18n();
const { t } = i18n;
const { applyAuthSystemConfig } = useAuthSystemConfig(i18n);

const description = computed(() => {
  if (errorMessage.value) return t("auth.oidcBind.invalidInvite");
  if (!invite.value) return t("auth.oidcBind.wait");
  return t("auth.oidcBind.selectProvider");
});

onMounted(loadInvite);

async function loadInvite() {
  isLoading.value = true;
  errorMessage.value = "";
  try {
    if (!token) throw new Error(t("auth.oidcBind.missingToken"));
    const res = await apiClient.get("/oidc/invite", {
      params: { token },
    });
    invite.value = res.data.data;
    await applyAuthSystemConfig(invite.value);
    if (!invite.value?.providers.length) {
      throw new Error(t("auth.oidcBind.noProviders"));
    }
  } catch (error: any) {
    await applyAuthSystemConfig(error?.response?.data?.data);
    errorMessage.value =
      error?.response?.data?.message ||
      error?.message ||
      t("auth.oidcBind.inviteExpired");
  } finally {
    isLoading.value = false;
  }
}

async function startBind(providerId: string) {
  if (isStarting.value) return;
  isStarting.value = true;
  activeProviderId.value = providerId;
  errorMessage.value = "";
  try {
    const res = await apiClient.post("/oidc/start", {
      provider_id: providerId,
      mode: "bind",
      invite_token: token,
      rememberMe: false,
    });
    const authorizationUrl = res.data?.data?.authorization_url;
    if (!authorizationUrl) {
      throw new Error(res.data?.message || t("auth.oidcBind.startFailed"));
    }
    window.location.assign(authorizationUrl);
  } catch (error: any) {
    errorMessage.value =
      error?.response?.data?.message ||
      error?.message ||
      t("auth.oidcBind.bindFailed");
    isStarting.value = false;
    activeProviderId.value = "";
  }
}
</script>
