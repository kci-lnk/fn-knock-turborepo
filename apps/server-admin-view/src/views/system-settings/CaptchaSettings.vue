<script setup lang="ts">
import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
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
import { CaptchaAPI } from "@/lib/api/config";
import { isPowDifficultyValid } from "../../lib/captcha-settings";
import type { CaptchaSettings as CaptchaSettingsModel } from "@frontend-core/captcha/types";
import PowCaptchaSettingsFields from "./captcha/PowCaptchaSettingsFields.vue";
import TurnstileCaptchaSettingsFields from "./captcha/TurnstileCaptchaSettingsFields.vue";

const { t } = useI18n();
const settings = ref<CaptchaSettingsModel | null>(null);
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
          <Label for="captcha-provider" class="text-base">{{
            t("admin.captchaSettings.typeLabel")
          }}</Label>
          <div class="text-sm text-muted-foreground">
            {{ t("admin.captchaSettings.typeDescription") }}
          </div>
        </div>
        <Select v-model="form.provider" :disabled="isSaving">
          <SelectTrigger
            id="captcha-provider"
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

      <PowCaptchaSettingsFields
        v-if="form.provider === 'pow'"
        v-model="form.pow"
        :disabled="isSaving"
      />

      <TurnstileCaptchaSettingsFields
        v-if="form.provider === 'turnstile'"
        v-model="form.turnstile"
        :disabled="isSaving"
      />
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
