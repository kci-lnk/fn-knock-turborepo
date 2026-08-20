<script setup lang="ts">
import { useId } from "vue";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";

const a11yId = useId();
const props = withDefaults(
  defineProps<{
    description: string;
    disabled?: boolean;
    modelValue: boolean;
    muted?: boolean;
    title: string;
  }>(),
  {
    disabled: false,
    muted: false,
  },
);

const emit = defineEmits<{
  change: [value: boolean];
}>();
</script>

<template>
  <section
    class="flex flex-col gap-4 p-6 sm:flex-row sm:items-center sm:justify-between"
    :class="muted ? 'bg-muted/10' : ''"
  >
    <div class="space-y-1 pr-6">
      <Label
        :for="`${a11yId}-switch`"
        class="cursor-pointer text-base font-medium"
        @click="emit('change', !props.modelValue)"
      >
        {{ title }}
      </Label>
      <div class="text-sm text-muted-foreground">
        {{ description }}
      </div>
    </div>
    <Switch
      :id="`${a11yId}-switch`"
      :model-value="modelValue"
      :disabled="disabled"
      @update:model-value="emit('change', $event === true)"
    />
  </section>
</template>
