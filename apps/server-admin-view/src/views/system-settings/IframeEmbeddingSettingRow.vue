<script setup lang="ts">
import { Button } from "@/components/ui/button";
import { useI18n } from "vue-i18n";

const props = defineProps<{
  disabled?: boolean;
  modelValue: boolean;
}>();

const emit = defineEmits<{
  change: [value: boolean];
}>();

const { t } = useI18n();

const selectOption = (value: boolean) => {
  if (!props.disabled && value !== props.modelValue) emit("change", value);
};
</script>

<template>
  <div
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <div class="text-base font-medium">
        {{ t("admin.featuresSettings.iframeEmbedding") }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.featuresSettings.iframeEmbeddingHint") }}
      </div>
    </div>
    <div
      role="group"
      :aria-label="t('admin.featuresSettings.iframeEmbedding')"
      class="inline-flex w-fit rounded-md border bg-background p-1"
    >
      <Button
        type="button"
        size="sm"
        :variant="modelValue ? 'ghost' : 'default'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="!modelValue"
        @click="selectOption(false)"
      >
        {{ t("admin.featuresSettings.iframeEmbeddingDenied") }}
      </Button>
      <Button
        type="button"
        size="sm"
        :variant="modelValue ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue"
        @click="selectOption(true)"
      >
        {{ t("admin.featuresSettings.iframeEmbeddingAllowed") }}
      </Button>
    </div>
  </div>
</template>
