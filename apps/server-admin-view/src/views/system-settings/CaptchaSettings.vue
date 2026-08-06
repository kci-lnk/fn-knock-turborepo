<script setup lang="ts">
import { computed, onMounted, reactive, ref, useId } from "vue";
import { useI18n } from "vue-i18n";
import { ExternalLink, Eye, EyeOff } from "lucide-vue-next";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Skeleton } from "@/components/ui/skeleton";
import { toast } from "@admin-shared/utils/toast";
import FloatingActionDock from "@admin-shared/components/common/FloatingActionDock.vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { CaptchaAPI } from "../../lib/api";
import {
  isPowDifficultyValid,
  POW_DIFFICULTY_MAX,
  POW_DIFFICULTY_MIN,
  POW_DIFFICULTY_STEP,
} from "../../lib/captcha-settings";
import type { CaptchaSettings as CaptchaSettingsModel } from "@frontend-core/captcha/types";

const a11yId = useId();

const { t } = useI18n();
const settings = ref<CaptchaSettingsModel | null>(null);
const turnstileSiteFieldId = "captcha-turnstile-public-token";
const turnstileSecretFieldId = "captcha-turnstile-private-token";
const powBaseFieldId = "captcha-pow-base-max-number";
const powUncommonFieldId = "captcha-pow-uncommon-max-number";
const turnstileGettingStartedUrl =
  "https://www.cloudflare-cn.com/application-services/products/turnstile/";
const isTurnstileSiteVisible = ref(false);
const isTurnstileSecretVisible = ref(false);
const form = reactive<CaptchaSettingsModel>({
  provider: "pow",
  widget_mode: "normal",
  pow: {
    base_max_number: 100_000,
    uncommon_location: {
      enabled: false,
      max_number: 300_000,
    },
  },
  turnstile: {
    site_key: "",
    secret_key: "",
  },
});

const { isPending: isLoading, run: runLoadSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.captchaSettings.loadFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.captchaSettings.loadDescription"),
      ),
    });
  },
});
const showLoadingSkeleton = useDelayedLoading(isLoading);
const { isPending: isSaving, run: runSaveSettings } = useAsyncAction({
  onError: (error) => {
    toast.error(t("admin.captchaSettings.saveFailed"), {
      description: extractErrorMessage(
        error,
        t("admin.captchaSettings.saveDescription"),
      ),
    });
  },
});

const isDirty = computed(() => {
  if (!settings.value) return false;
  return (
    settings.value.provider !== form.provider ||
    settings.value.pow.base_max_number !== form.pow.base_max_number ||
    settings.value.pow.uncommon_location.enabled !==
      form.pow.uncommon_location.enabled ||
    settings.value.pow.uncommon_location.max_number !==
      form.pow.uncommon_location.max_number ||
    settings.value.turnstile.site_key !== form.turnstile.site_key ||
    settings.value.turnstile.secret_key !== form.turnstile.secret_key
  );
});

const applyFromSettings = (data: CaptchaSettingsModel) => {
  settings.value = data;
  form.provider = data.provider;
  form.widget_mode = "normal";
  form.pow.base_max_number = data.pow.base_max_number;
  form.pow.uncommon_location.enabled = data.pow.uncommon_location.enabled;
  form.pow.uncommon_location.max_number = data.pow.uncommon_location.max_number;
  form.turnstile.site_key = data.turnstile.site_key;
  form.turnstile.secret_key = data.turnstile.secret_key;
};

const fetchSettings = async () => {
  await runLoadSettings(async () => {
    const data = await CaptchaAPI.getSettings();
    applyFromSettings(data);
  });
};

const resetForm = () => {
  if (settings.value) applyFromSettings(settings.value);
};

const saveSettings = async () => {
  const baseMaxNumber = Number(form.pow.base_max_number);
  const uncommonMaxNumber = Number(form.pow.uncommon_location.max_number);
  if (!isPowDifficultyValid(baseMaxNumber, uncommonMaxNumber)) {
    toast.error(t("admin.captchaSettings.powDifficultyInvalidTitle"), {
      description: t("admin.captchaSettings.powDifficultyInvalidDescription"),
    });
    return;
  }

  if (form.provider === "turnstile") {
    if (!form.turnstile.site_key.trim() || !form.turnstile.secret_key.trim()) {
      toast.error(t("admin.captchaSettings.incompleteTitle"), {
        description: t("admin.captchaSettings.incompleteDescription"),
      });
      return;
    }
  }

  await runSaveSettings(
    () =>
      CaptchaAPI.updateSettings({
        provider: form.provider,
        widget_mode: "normal",
        pow: {
          base_max_number: baseMaxNumber,
          uncommon_location: {
            enabled: form.pow.uncommon_location.enabled,
            max_number: uncommonMaxNumber,
          },
        },
        turnstile: {
          site_key: form.turnstile.site_key.trim(),
          secret_key: form.turnstile.secret_key.trim(),
        },
      }),
    {
      onSuccess: (data) => {
        applyFromSettings(data);
        toast.success(t("admin.captchaSettings.updated"));
      },
    },
  );
};

onMounted(fetchSettings);
</script>

<template>
  <Card>
    <CardHeader>
      <CardTitle class="text-md">{{
        t("admin.captchaSettings.title")
      }}</CardTitle>
      <CardDescription class="mt-1.5">
        {{ t("admin.captchaSettings.description") }}
      </CardDescription>
    </CardHeader>

    <CardContent v-if="isLoading && showLoadingSkeleton" class="border-t p-0">
      <div class="space-y-4 p-6">
        <Skeleton class="h-6 w-1/3" />
        <Skeleton class="h-4 w-2/3" />
      </div>
    </CardContent>

    <CardContent v-else-if="!isLoading" class="border-t p-0 divide-y">
      <div
        class="flex flex-col gap-4 bg-muted/10 p-6 sm:flex-row sm:items-center sm:justify-between"
      >
        <div class="min-w-0 space-y-1 sm:flex-1 sm:pr-6">
          <Label :for="`${a11yId}-captchasettings-1`" class="text-base">{{
            t("admin.captchaSettings.typeLabel")
          }}</Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.captchaSettings.typeDescription") }}
          </div>
        </div>
        <Select v-model="form.provider" :disabled="isSaving">
          <SelectTrigger
            :id="`${a11yId}-captchasettings-1`"
            class="w-full sm:shrink-0"
            style="width: min(100%, 300px)"
          >
            <SelectValue
              :placeholder="t('admin.captchaSettings.typePlaceholder')"
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="pow">{{
              t("admin.captchaSettings.powOption")
            }}</SelectItem>
            <SelectItem value="turnstile">Cloudflare Turnstile</SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div
        v-if="form.provider === 'pow'"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="powBaseFieldId">
              {{ t("admin.captchaSettings.powBaseDifficulty") }}
            </Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              {{ t("admin.captchaSettings.powBaseDifficultyDescription") }}
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <Input
              :id="powBaseFieldId"
              v-model.number="form.pow.base_max_number"
              type="number"
              :min="POW_DIFFICULTY_MIN"
              :max="POW_DIFFICULTY_MAX"
              :step="POW_DIFFICULTY_STEP"
              inputmode="numeric"
              :disabled="isSaving"
            />
          </div>
        </div>

        <div class="flex items-center justify-between gap-4 p-6">
          <div class="space-y-1 pr-6">
            <Label
              :for="`${a11yId}-captcha-pow-uncommon-location`"
              class="cursor-pointer text-base font-medium"
            >
              {{ t("admin.captchaSettings.powUncommonLocation") }}
            </Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              {{ t("admin.captchaSettings.powUncommonLocationDescription") }}
            </div>
          </div>
          <Switch
            :id="`${a11yId}-captcha-pow-uncommon-location`"
            v-model="form.pow.uncommon_location.enabled"
            :disabled="isSaving"
          />
        </div>

        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="powUncommonFieldId">
              {{ t("admin.captchaSettings.powUncommonDifficulty") }}
            </Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              {{ t("admin.captchaSettings.powUncommonDifficultyDescription") }}
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <Input
              :id="powUncommonFieldId"
              v-model.number="form.pow.uncommon_location.max_number"
              type="number"
              :min="Math.max(POW_DIFFICULTY_MIN, form.pow.base_max_number)"
              :max="POW_DIFFICULTY_MAX"
              :step="POW_DIFFICULTY_STEP"
              inputmode="numeric"
              :disabled="isSaving"
            />
          </div>
        </div>
      </div>

      <div
        v-if="form.provider === 'turnstile'"
        class="divide-y animate-in fade-in slide-in-from-top-2 duration-300"
      >
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
                :href="turnstileGettingStartedUrl"
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

        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="turnstileSiteFieldId">
              {{ t("admin.captchaSettings.siteKey") }}
            </Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              {{ t("admin.captchaSettings.siteKeyDescription") }}
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <div class="relative">
              <Input
                :id="turnstileSiteFieldId"
                v-model="form.turnstile.site_key"
                :type="isTurnstileSiteVisible ? 'text' : 'password'"
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
                :disabled="isSaving"
              />
              <button
                type="button"
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                :aria-label="
                  isTurnstileSiteVisible
                    ? t('admin.captchaSettings.hideSiteKey')
                    : t('admin.captchaSettings.showSiteKey')
                "
                :disabled="isSaving"
                @click="isTurnstileSiteVisible = !isTurnstileSiteVisible"
              >
                <component
                  :is="isTurnstileSiteVisible ? EyeOff : Eye"
                  class="h-4 w-4"
                />
              </button>
            </div>
          </div>
        </div>

        <div class="captcha-key-row">
          <div class="captcha-key-copy min-w-0 space-y-1">
            <Label class="text-base" :for="turnstileSecretFieldId">
              {{ t("admin.captchaSettings.secretKey") }}
            </Label>
            <div class="text-sm leading-relaxed text-muted-foreground">
              {{ t("admin.captchaSettings.secretKeyDescription") }}
            </div>
          </div>
          <div class="captcha-key-input-wrap w-full">
            <div class="relative">
              <Input
                :id="turnstileSecretFieldId"
                v-model="form.turnstile.secret_key"
                :type="isTurnstileSecretVisible ? 'text' : 'password'"
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
                :disabled="isSaving"
              />
              <button
                type="button"
                class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-50"
                :aria-label="
                  isTurnstileSecretVisible
                    ? t('admin.captchaSettings.hideSecretKey')
                    : t('admin.captchaSettings.showSecretKey')
                "
                :disabled="isSaving"
                @click="isTurnstileSecretVisible = !isTurnstileSecretVisible"
              >
                <component
                  :is="isTurnstileSecretVisible ? EyeOff : Eye"
                  class="h-4 w-4"
                />
              </button>
            </div>
          </div>
        </div>
      </div>
    </CardContent>

    <CardContent v-else class="min-h-[200px]" aria-hidden="true" />

    <FloatingActionDock
      :active="isDirty"
      inline-class="flex items-center justify-between rounded-b-xl border-t bg-muted/20 p-6"
    >
      <template #inline>
        <div class="text-sm text-muted-foreground">
          <span v-if="isDirty">{{ t("admin.captchaSettings.dirty") }}</span>
          <span v-else>{{ t("admin.captchaSettings.clean") }}</span>
        </div>
        <div class="flex gap-3">
          <Button
            variant="outline"
            @click="resetForm"
            :disabled="!isDirty || isSaving"
          >
            {{ t("admin.captchaSettings.discard") }}
          </Button>
          <Button :disabled="!isDirty || isSaving" @click="saveSettings">
            <span
              v-if="isSaving"
              class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
            ></span>
            {{ t("admin.captchaSettings.saveChanges") }}
          </Button>
        </div>
      </template>

      <template #floating>
        <Button
          variant="outline"
          @click="resetForm"
          :disabled="!isDirty || isSaving"
        >
          {{ t("admin.captchaSettings.discard") }}
        </Button>
        <Button :disabled="!isDirty || isSaving" @click="saveSettings">
          <span
            v-if="isSaving"
            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-foreground"
          ></span>
          {{ t("admin.captchaSettings.saveChanges") }}
        </Button>
      </template>
    </FloatingActionDock>
  </Card>
</template>

<style scoped>
.captcha-key-row {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  padding: 1.5rem;
}

@media (min-width: 768px) {
  .captcha-key-row {
    display: grid;
    grid-template-columns: 320px minmax(0, 1fr);
    align-items: start;
    column-gap: 2rem;
  }

  .captcha-key-copy {
    padding-top: 0.25rem;
  }

  .captcha-key-input-wrap {
    width: 88%;
    justify-self: end;
    margin-top: 0.875rem;
  }
}
</style>
