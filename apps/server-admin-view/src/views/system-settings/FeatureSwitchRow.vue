<script setup lang="ts">
import { computed, useId } from "vue";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

const a11yId = useId();

const props = withDefaults(
  defineProps<{
    description: string;
    disabled?: boolean;
    disabledReason?: string;
    error?: string;
    modelValue: boolean;
    title: string;
    available?: boolean;
  }>(),
  {
    available: true,
    disabled: false,
    disabledReason: "",
    error: "",
  },
);

const emit = defineEmits<{
  change: [value: boolean];
}>();

const isInteractive = computed(() => props.available && !props.disabled);
const displayedValue = computed(() =>
  props.available ? props.modelValue : false,
);
const titleClass = computed(() => {
  if (!isInteractive.value) return "cursor-not-allowed text-zinc-500";
  if (props.error) return "cursor-pointer text-red-600";
  return "cursor-pointer";
});
const descriptionClass = computed(() => {
  if (!props.available) return "text-zinc-500";
  if (props.error) return "text-red-600";
  return "text-muted-foreground";
});

const requestChange = (value = !props.modelValue) => {
  if (!isInteractive.value) return;
  emit("change", value);
};
</script>

<template>
  <div class="flex items-center justify-between bg-muted/10 p-6">
    <div class="space-y-1 pr-6">
      <Label
        :for="`${a11yId}-featureswitchrow-1`"
        class="text-base font-medium"
        :class="titleClass"
      >
        {{ title }}
      </Label>
      <div class="text-sm" :class="descriptionClass">
        {{ description }}
      </div>
      <div
        v-if="error || (!available && disabledReason)"
        :id="`${a11yId}-featureswitchrow-status`"
        class="text-xs leading-5"
        :class="error ? 'text-red-600' : 'text-zinc-500'"
        :role="error ? 'alert' : undefined"
      >
        {{ error || disabledReason }}
      </div>
    </div>
    <Switch
      :id="`${a11yId}-featureswitchrow-1`"
      :model-value="displayedValue"
      :disabled="!available || disabled"
      :aria-describedby="
        error || (!available && disabledReason)
          ? `${a11yId}-featureswitchrow-status`
          : undefined
      "
      :aria-invalid="Boolean(error)"
      @update:model-value="requestChange($event === true)"
    />
  </div>
</template>
