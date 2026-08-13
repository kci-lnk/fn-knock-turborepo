<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import {
  ensureUncommonDifficultyAtLeastBase,
  isPowDifficultyPreset,
  POW_DIFFICULTY_STANDARD,
  POW_DIFFICULTY_VERY_HARD,
} from "@/lib/captcha-settings";
import type { PowCaptchaConfig } from "@frontend-core/captcha/types";
import CaptchaConfigField from "./CaptchaConfigField.vue";

defineProps<{ disabled: boolean }>();
const model = defineModel<PowCaptchaConfig>({ required: true });
const { t } = useI18n();
const baseFieldId = "captcha-pow-base-max-number";
const uncommonFieldId = "captcha-pow-uncommon-max-number";

const baseDifficultySelection = computed({
  get: () => String(model.value.base_max_number),
  set: (value: string) => {
    const difficulty = Number(value);
    if (!isPowDifficultyPreset(difficulty)) return;
    model.value.base_max_number = difficulty;
    model.value.uncommon_location.max_number =
      ensureUncommonDifficultyAtLeastBase(
        difficulty,
        model.value.uncommon_location.max_number,
      );
  },
});

const uncommonDifficultySelection = computed({
  get: () => String(model.value.uncommon_location.max_number),
  set: (value: string) => {
    const difficulty = Number(value);
    if (
      !isPowDifficultyPreset(difficulty) ||
      difficulty < model.value.base_max_number
    ) {
      return;
    }
    model.value.uncommon_location.max_number = difficulty;
  },
});
</script>

<template>
  <div class="divide-y animate-in fade-in slide-in-from-top-2 duration-300">
    <CaptchaConfigField control-class="md:w-[300px]">
      <template #copy>
        <Label class="text-base" :for="baseFieldId">
          {{ t("admin.captchaSettings.powBaseDifficulty") }}
        </Label>
        <div class="text-sm leading-relaxed text-muted-foreground">
          {{ t("admin.captchaSettings.powBaseDifficultyDescription") }}
        </div>
      </template>
      <Select v-model="baseDifficultySelection" :disabled="disabled">
        <SelectTrigger :id="baseFieldId" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem :value="String(POW_DIFFICULTY_STANDARD)">
            {{ t("admin.captchaSettings.powDifficultyStandard") }}
          </SelectItem>
          <SelectItem :value="String(POW_DIFFICULTY_VERY_HARD)">
            {{ t("admin.captchaSettings.powDifficultyVeryHard") }}
          </SelectItem>
          <SelectItem
            v-if="!isPowDifficultyPreset(model.base_max_number)"
            :value="String(model.base_max_number)"
          >
            {{ t("admin.captchaSettings.powDifficultyCustom") }}
          </SelectItem>
        </SelectContent>
      </Select>
    </CaptchaConfigField>

    <div class="flex items-center justify-between gap-4 p-6">
      <div class="space-y-1 pr-6">
        <Label
          for="captcha-pow-uncommon-location"
          class="cursor-pointer text-base font-medium"
        >
          {{ t("admin.captchaSettings.powUncommonLocation") }}
        </Label>
        <div class="text-sm leading-relaxed text-muted-foreground">
          {{ t("admin.captchaSettings.powUncommonLocationDescription") }}
        </div>
      </div>
      <Switch
        id="captcha-pow-uncommon-location"
        v-model="model.uncommon_location.enabled"
        :disabled="disabled"
      />
    </div>

    <CaptchaConfigField
      v-if="model.uncommon_location.enabled"
      control-class="md:w-[300px]"
      class="animate-in fade-in slide-in-from-top-2 duration-300"
    >
      <template #copy>
        <Label class="text-base" :for="uncommonFieldId">
          {{ t("admin.captchaSettings.powUncommonDifficulty") }}
        </Label>
        <div class="text-sm leading-relaxed text-muted-foreground">
          {{ t("admin.captchaSettings.powUncommonDifficultyDescription") }}
        </div>
      </template>
      <Select v-model="uncommonDifficultySelection" :disabled="disabled">
        <SelectTrigger :id="uncommonFieldId" class="w-full">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            :value="String(POW_DIFFICULTY_STANDARD)"
            :disabled="POW_DIFFICULTY_STANDARD < model.base_max_number"
          >
            {{ t("admin.captchaSettings.powDifficultyStandard") }}
          </SelectItem>
          <SelectItem
            :value="String(POW_DIFFICULTY_VERY_HARD)"
            :disabled="POW_DIFFICULTY_VERY_HARD < model.base_max_number"
          >
            {{ t("admin.captchaSettings.powDifficultyVeryHard") }}
          </SelectItem>
          <SelectItem
            v-if="!isPowDifficultyPreset(model.uncommon_location.max_number)"
            :value="String(model.uncommon_location.max_number)"
          >
            {{ t("admin.captchaSettings.powDifficultyCustom") }}
          </SelectItem>
        </SelectContent>
      </Select>
    </CaptchaConfigField>
  </div>
</template>
