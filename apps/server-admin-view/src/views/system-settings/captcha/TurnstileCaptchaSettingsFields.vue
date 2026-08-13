<script setup lang="ts">
import { ref } from "vue";
import { useI18n } from "vue-i18n";
import { ExternalLink, Eye, EyeOff } from "lucide-vue-next";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import type { TurnstileCaptchaConfig } from "@frontend-core/captcha/types";
import CaptchaConfigField from "./CaptchaConfigField.vue";

defineProps<{ disabled: boolean }>();
const model = defineModel<TurnstileCaptchaConfig>({ required: true });
const { t } = useI18n();
const siteFieldId = "captcha-turnstile-public-token";
const secretFieldId = "captcha-turnstile-private-token";
const gettingStartedUrl =
  "https://www.cloudflare-cn.com/application-services/products/turnstile/";
const isSiteVisible = ref(false);
const isSecretVisible = ref(false);
</script>

<template>
  <div class="divide-y animate-in fade-in slide-in-from-top-2 duration-300">
    <div class="grid gap-4 bg-muted/10 p-6">
      <div
        class="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between"
      >
        <div class="space-y-1">
          <div class="text-base font-medium">
            {{ t("admin.captchaSettings.turnstileSetupTitle") }}
          </div>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.captchaSettings.turnstileSetupDescription") }}
          </div>
        </div>
        <Button as-child variant="outline" class="shrink-0">
          <a
            :href="gettingStartedUrl"
            target="_blank"
            rel="noreferrer noopener"
          >
            <ExternalLink class="mr-2 h-4 w-4" />
            {{ t("admin.captchaSettings.openTurnstile") }}
          </a>
        </Button>
      </div>
      <div class="grid gap-2 text-sm text-muted-foreground">
        <div>{{ t("admin.captchaSettings.stepLogin") }}</div>
        <div>{{ t("admin.captchaSettings.stepCreate") }}</div>
        <div>{{ t("admin.captchaSettings.stepCopy") }}</div>
      </div>
    </div>

    <CaptchaConfigField>
      <template #copy>
        <Label class="text-base" :for="siteFieldId">
          {{ t("admin.captchaSettings.siteKey") }}
        </Label>
        <div class="text-sm leading-relaxed text-muted-foreground">
          {{ t("admin.captchaSettings.siteKeyDescription") }}
        </div>
      </template>
      <div class="relative">
        <Input
          :id="siteFieldId"
          v-model="model.site_key"
          :type="isSiteVisible ? 'text' : 'password'"
          name="captchaPublicToken"
          autocomplete="off"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
          data-form-type="other"
          data-1p-ignore="true"
          data-lpignore="true"
          data-bwignore="true"
          :placeholder="t('admin.captchaSettings.siteKeyPlaceholder')"
          class="pr-10"
          :disabled="disabled"
        />
        <button
          type="button"
          class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
          :aria-label="
            isSiteVisible
              ? t('admin.captchaSettings.hideSiteKey')
              : t('admin.captchaSettings.showSiteKey')
          "
          :disabled="disabled"
          @click="isSiteVisible = !isSiteVisible"
        >
          <component :is="isSiteVisible ? EyeOff : Eye" class="h-4 w-4" />
        </button>
      </div>
    </CaptchaConfigField>

    <CaptchaConfigField>
      <template #copy>
        <Label class="text-base" :for="secretFieldId">
          {{ t("admin.captchaSettings.secretKey") }}
        </Label>
        <div class="text-sm leading-relaxed text-muted-foreground">
          {{ t("admin.captchaSettings.secretKeyDescription") }}
        </div>
      </template>
      <div class="relative">
        <Input
          :id="secretFieldId"
          v-model="model.secret_key"
          :type="isSecretVisible ? 'text' : 'password'"
          name="captchaPrivateToken"
          autocomplete="new-password"
          autocapitalize="off"
          autocorrect="off"
          spellcheck="false"
          data-form-type="other"
          data-1p-ignore="true"
          data-lpignore="true"
          data-bwignore="true"
          :placeholder="t('admin.captchaSettings.secretKeyPlaceholder')"
          class="pr-10"
          :disabled="disabled"
        />
        <button
          type="button"
          class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
          :aria-label="
            isSecretVisible
              ? t('admin.captchaSettings.hideSecretKey')
              : t('admin.captchaSettings.showSecretKey')
          "
          :disabled="disabled"
          @click="isSecretVisible = !isSecretVisible"
        >
          <component :is="isSecretVisible ? EyeOff : Eye" class="h-4 w-4" />
        </button>
      </div>
    </CaptchaConfigField>
  </div>
</template>
