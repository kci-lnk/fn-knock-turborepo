<script setup lang="ts">
import { Button } from "@/components/ui/button";
import type { DateTimeDisplayMode } from "@/types";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  disabled?: boolean;
  modelValue: DateTimeDisplayMode;
}>();

const emit = defineEmits<{
  change: [value: DateTimeDisplayMode];
}>();

const selectMode = (value: DateTimeDisplayMode) => {
  if (!props.disabled && value !== props.modelValue) emit("change", value);
};

const { t } = useI18n();
</script>

<template>
  <div
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <div class="text-base font-medium">
        {{ t("admin.featuresSettings.dateTimeDisplayMode") }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.featuresSettings.dateTimeDisplayModeHint") }}
      </div>
    </div>
    <div
      role="group"
      :aria-label="t('admin.featuresSettings.dateTimeDisplayMode')"
      class="inline-flex w-fit rounded-md border bg-background p-1"
    >
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'human_friendly' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'human_friendly'"
        @click="selectMode('human_friendly')"
      >
        {{ t("admin.featuresSettings.dateTimeDisplayHumanFriendly") }}
      </Button>
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'full' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'full'"
        @click="selectMode('full')"
      >
        {{ t("admin.featuresSettings.dateTimeDisplayFull") }}
      </Button>
    </div>
  </div>
</template>
