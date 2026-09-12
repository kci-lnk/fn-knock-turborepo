<script setup lang="ts">
import { ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { WAFConfig } from "@/types/waf";

const props = defineProps<{ config: WAFConfig; disabled: boolean }>();
const emit = defineEmits<{
  save: [
    value: {
      violation_rate_limit_enabled: boolean;
      violation_rate_limit_capacity: number;
      violation_rate_limit_refill_seconds: number;
    },
  ];
}>();
const { t } = useI18n();
const id = useId();
const presets = {
  strict: { capacity: 1, refill: 120 },
  normal: { capacity: 5, refill: 60 },
  relaxed: { capacity: 20, refill: 10 },
} as const;
type Level = "off" | keyof typeof presets | "custom";
const options = ["off", "strict", "normal", "relaxed"] as const;
const levelFromConfig = (config: WAFConfig): Level => {
  if (!config.violation_rate_limit_enabled) return "off";
  const capacity = config.violation_rate_limit_capacity ?? 5;
  const refill = config.violation_rate_limit_refill_seconds ?? 60;
  return (
    options.find(
      (level) =>
        level !== "off" &&
        presets[level].capacity === capacity &&
        presets[level].refill === refill,
    ) ?? "custom"
  );
};
const selected = ref<Level>(levelFromConfig(props.config));
const pending = ref(false);
watch(
  () => props.config,
  (config) => {
    selected.value = levelFromConfig(config);
    pending.value = false;
  },
);
const changeLevel = (value: unknown) => {
  if (
    props.disabled ||
    pending.value ||
    !options.some((level) => level === value) ||
    value === selected.value
  )
    return;
  const level = value as (typeof options)[number];
  const parameters =
    level === "off"
      ? {
          capacity: props.config.violation_rate_limit_capacity ?? 5,
          refill: props.config.violation_rate_limit_refill_seconds ?? 60,
        }
      : presets[level];
  selected.value = level;
  pending.value = true;
  emit("save", {
    violation_rate_limit_enabled: level !== "off",
    violation_rate_limit_capacity: parameters.capacity,
    violation_rate_limit_refill_seconds: parameters.refill,
  });
};
</script>

<template>
  <section
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_10rem] sm:items-center sm:gap-6"
  >
    <div class="space-y-1">
      <Label :for="id" class="text-base">{{
        t("admin.wafSettings.violationRate.title")
      }}</Label>
      <p :id="`${id}-description`" class="text-sm text-muted-foreground">
        {{ t("admin.wafSettings.violationRate.description") }}
      </p>
      <p :id="`${id}-hint`" class="text-xs text-muted-foreground">
        {{ t(`admin.wafSettings.violationRate.${selected}Hint`) }}
      </p>
    </div>
    <Select
      :model-value="selected"
      :disabled="disabled || pending"
      @update:model-value="changeLevel"
    >
      <SelectTrigger
        :id="id"
        :aria-describedby="`${id}-description ${id}-hint`"
        :aria-busy="pending"
      >
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem v-for="level in options" :key="level" :value="level">
          {{ t(`admin.wafSettings.violationRate.${level}`) }}
        </SelectItem>
        <SelectItem v-if="selected === 'custom'" value="custom" disabled>
          {{ t("admin.wafSettings.violationRate.custom") }}
        </SelectItem>
      </SelectContent>
    </Select>
  </section>
</template>
