<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

type DurationField = {
  unit: string;
  value: number;
};

type DurationUnitOption = {
  labelKey: string;
  value: string;
};

const props = withDefaults(
  defineProps<{
    description: string;
    disabled?: boolean;
    framed?: boolean;
    modelValue: DurationField;
    summary?: string;
    title: string;
    units: DurationUnitOption[];
  }>(),
  {
    disabled: false,
    framed: false,
    summary: "",
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: DurationField];
}>();

const { t } = useI18n();

const valueModel = computed({
  get: () => props.modelValue.value,
  set: (value: string | number) => {
    emit("update:modelValue", {
      ...props.modelValue,
      value: Number(value),
    });
  },
});

const unitModel = computed({
  get: () => props.modelValue.unit,
  set: (unit: string) => {
    emit("update:modelValue", {
      ...props.modelValue,
      unit,
    });
  },
});
</script>

<template>
  <div
    :class="[
      'grid gap-3 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4',
      framed ? 'rounded-xl border bg-muted/15 p-4' : 'p-6',
    ]"
  >
    <div class="space-y-1 pr-6">
      <div class="text-base font-medium">
        {{ title }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ description }}
      </div>
    </div>
    <div class="flex shrink-0 items-center gap-2">
      <Input
        v-model.number="valueModel"
        :aria-label="title"
        type="number"
        min="1"
        step="1"
        class="w-24 text-center"
        :disabled="disabled"
      />
      <Select v-model="unitModel" :disabled="disabled">
        <SelectTrigger :aria-label="title" class="w-[110px]">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="unit in units"
            :key="unit.value"
            :value="unit.value"
          >
            {{ t(unit.labelKey) }}
          </SelectItem>
        </SelectContent>
      </Select>
    </div>
    <div
      v-if="summary"
      class="-mt-1 text-xs text-muted-foreground sm:col-span-2"
    >
      {{ summary }}
    </div>
  </div>
</template>
