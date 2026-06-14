<template>
  <div class="auth-safe-shell flex flex-col bg-muted/40">
    <div class="flex flex-1 items-center justify-center">
      <Card class="w-full max-w-sm">
        <CardHeader>
          <CardTitle class="text-2xl text-center">
            {{ t("auth.oidcBind.title") }}
          </CardTitle>
          <CardDescription class="text-center">
            {{ description }}
          </CardDescription>
        </CardHeader>
        <CardContent class="space-y-4">
          <div
            v-if="isLoading"
            class="py-8 text-center text-sm text-muted-foreground"
          >
            {{ t("auth.oidcBind.checkingInvite") }}
          </div>
          <div
            v-else-if="errorMessage"
            class="rounded-lg border border-destructive/30 bg-destructive/5 px-4 py-3 text-sm text-destructive"
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
              {{ t("auth.oidcBind.useProvider", { provider: provider.name }) }}
            </Button>
          </div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { apiClient } from "@/lib/api";
import { normalizeLocale, type LocaleConfig } from "@fn-knock/i18n";
import { applyDocumentLocale } from "@fn-knock/i18n/vue";

type InviteDetails = {
  locale: LocaleConfig;
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
const { t, locale } = useI18n();

const applySystemLocale = (value: string | null | undefined) => {
  const next = normalizeLocale(value) ?? "zh-CN";
  locale.value = next;
  applyDocumentLocale(next);
};

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
    applySystemLocale(invite.value?.locale?.default_locale);
    if (!invite.value?.providers.length) {
      throw new Error(t("auth.oidcBind.noProviders"));
    }
  } catch (error: any) {
    applySystemLocale(error?.response?.data?.data?.locale?.default_locale);
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
