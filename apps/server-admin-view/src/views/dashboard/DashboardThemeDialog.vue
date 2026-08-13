<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { Check } from "lucide-vue-next";
import { toast } from "@admin-shared/utils/toast";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  THEME_COLOR_PRESETS,
  normalizeAppearanceConfig,
  type ThemeColorPresetKey,
} from "@frontend-core/appearance";
import { useConfigStore } from "@/store/config";

const open = defineModel<boolean>("open", { required: true });
const { t } = useI18n();
const configStore = useConfigStore();
const isSaving = ref(false);
const options = THEME_COLOR_PRESETS.map((preset) => ({
  ...preset,
  labelKey: `admin.dashboard.theme.presets.${preset.key}`,
}));
const activePreset = computed(
  () =>
    normalizeAppearanceConfig(configStore.config?.appearance)
      .theme_color_preset,
);

const getErrorMessage = (error: unknown, fallback: string) => {
  const value = error as {
    response?: { data?: { message?: string } };
    message?: string;
  };
  return value?.response?.data?.message || value?.message || fallback;
};

const selectPreset = async (preset: ThemeColorPresetKey) => {
  if (preset === activePreset.value || isSaving.value) return;
  isSaving.value = true;
  try {
    await configStore.saveAppearanceConfig({ theme_color_preset: preset });
    open.value = false;
  } catch (error) {
    toast.error(t("admin.dashboard.theme.saveFailed"), {
      description: getErrorMessage(error, t("common.tryLater")),
    });
  } finally {
    isSaving.value = false;
  }
};
</script>

<template>
  <Dialog v-model:open="open">
    <DialogContent class="sm:max-w-[460px]">
      <DialogHeader>
        <DialogTitle>{{ t("admin.dashboard.theme.title") }}</DialogTitle>
        <DialogDescription>
          {{ t("admin.dashboard.theme.description") }}
        </DialogDescription>
      </DialogHeader>

      <div class="grid gap-2">
        <Button
          v-for="preset in options"
          :key="preset.key"
          :data-theme-preset="preset.key"
          type="button"
          variant="outline"
          class="h-auto justify-start gap-3 px-3 py-3 text-left"
          :class="
            preset.key === activePreset
              ? 'border-primary bg-primary/5 ring-1 ring-primary/20'
              : 'border-border/70 hover:border-primary/35'
          "
          :disabled="isSaving"
          @click="selectPreset(preset.key)"
        >
          <span
            class="size-5 shrink-0 rounded-full border border-border shadow-sm"
            :style="{ backgroundColor: preset.color }"
          />
          <span class="min-w-0 flex-1 text-sm font-medium">
            {{ t(preset.labelKey) }}
          </span>
          <Check
            v-if="preset.key === activePreset"
            class="h-4 w-4 shrink-0 text-primary"
          />
        </Button>
      </div>
    </DialogContent>
  </Dialog>
</template>
