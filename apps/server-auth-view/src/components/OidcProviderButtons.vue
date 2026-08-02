<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { CircleUserRound, Cloud, Github } from "lucide-vue-next";
import type { AuthOidcProvider } from "@frontend-core/auth/types";
import { Button } from "@/components/ui/button";
import QqIcon from "@/components/QqIcon.vue";

defineProps<{
  activeProviderId: string;
  disabled: boolean;
  isLoading: boolean;
  providers: AuthOidcProvider[];
  showDivider?: boolean;
}>();

const emit = defineEmits<{
  login: [providerId: string];
}>();

const { t } = useI18n();

type ProviderIconKind =
  "qq" | "github" | "google" | "microsoft" | "custom_oidc" | "generic";

const providerIconKind = (provider: AuthOidcProvider): ProviderIconKind => {
  const token = `${provider.type || ""} ${provider.name || ""} ${
    provider.protocol || ""
  }`.toLowerCase();
  if (provider.type === "fnknock_qq" || token.includes(" tencent qq")) {
    return "qq";
  }
  if (token.includes("github")) return "github";
  if (token.includes("google")) return "google";
  if (token.includes("microsoft") || token.includes("azure")) {
    return "microsoft";
  }
  if (token.includes("custom") || token.includes("oidc")) return "custom_oidc";
  return "generic";
};
</script>

<template>
  <div class="w-full space-y-2">
    <div
      v-if="showDivider"
      class="flex w-full items-center gap-3 text-sm text-muted-foreground"
      aria-hidden="true"
    >
      <div class="h-px flex-1 bg-border" />
      <span class="shrink-0">{{ t("auth.or") }}</span>
      <div class="h-px flex-1 bg-border" />
    </div>
    <Button
      v-for="provider in providers"
      :key="provider.id"
      type="button"
      variant="outline"
      class="w-full"
      :disabled="disabled"
      @click="emit('login', provider.id)"
    >
      <span
        v-if="activeProviderId === provider.id && isLoading"
        class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-primary border-t-transparent"
      />
      <Github
        v-else-if="providerIconKind(provider) === 'github'"
        class="size-4"
        aria-hidden="true"
      />
      <QqIcon
        v-else-if="providerIconKind(provider) === 'qq'"
        class="size-4 text-[#1ebafc]"
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
        <span class="bg-[#f25022]" />
        <span class="bg-[#7fba00]" />
        <span class="bg-[#00a4ef]" />
        <span class="bg-[#ffb900]" />
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
</template>
