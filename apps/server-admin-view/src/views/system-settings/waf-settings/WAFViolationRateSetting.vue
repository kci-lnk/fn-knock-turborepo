<script setup lang="ts">
import { computed, reactive, ref, useId, watch } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import WAFSettingSwitchRow from "./WAFSettingSwitchRow.vue";
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
const form = reactive({
  enabled: false,
  capacity: 5 as number | string,
  refill: 60 as number | string,
});
const awaitingSave = ref(false);
watch(
  () => props.config,
  (config, previous) => {
    // Other WAF controls also refresh details; retain this form's unsaved draft.
    if (
      !awaitingSave.value &&
      previous &&
      config.violation_rate_limit_enabled ===
        previous.violation_rate_limit_enabled &&
      config.violation_rate_limit_capacity ===
        previous.violation_rate_limit_capacity &&
      config.violation_rate_limit_refill_seconds ===
        previous.violation_rate_limit_refill_seconds
    )
      return;
    awaitingSave.value = false;
    form.enabled = config.violation_rate_limit_enabled ?? false;
    form.capacity = config.violation_rate_limit_capacity ?? 5;
    form.refill = config.violation_rate_limit_refill_seconds ?? 60;
  },
  { immediate: true },
);
const valid = computed(
  () =>
    !form.enabled ||
    (Number.isInteger(Number(form.capacity)) &&
      Number(form.capacity) >= 1 &&
      Number(form.capacity) <= 10000 &&
      Number.isInteger(Number(form.refill)) &&
      Number(form.refill) >= 1 &&
      Number(form.refill) <= 86400),
);
const setEnabled = (enabled: boolean) => {
  if (!props.disabled) form.enabled = enabled;
};
const save = () => {
  if (!valid.value || props.disabled) return;
  awaitingSave.value = true;
  emit("save", {
    violation_rate_limit_enabled: form.enabled,
    violation_rate_limit_capacity: form.enabled
      ? Number(form.capacity)
      : (props.config.violation_rate_limit_capacity ?? 5),
    violation_rate_limit_refill_seconds: form.enabled
      ? Number(form.refill)
      : (props.config.violation_rate_limit_refill_seconds ?? 60),
  });
};
</script>

<template>
  <section>
    <WAFSettingSwitchRow
      :title="t('admin.wafSettings.violationRate.title')"
      :description="t('admin.wafSettings.violationRate.description')"
      :model-value="form.enabled"
      :disabled="disabled"
      @change="setEnabled"
    />
    <form class="space-y-4 px-6 pb-6" @submit.prevent="save">
      <div v-if="form.enabled" class="grid gap-4 sm:grid-cols-2">
        <div class="space-y-2">
          <Label :for="`${id}-capacity`">{{
            t("admin.wafSettings.violationRate.capacity")
          }}</Label>
          <Input
            :id="`${id}-capacity`"
            v-model="form.capacity"
            type="number"
            min="1"
            max="10000"
            step="1"
            required
            :disabled="disabled"
          />
        </div>
        <div class="space-y-2">
          <Label :for="`${id}-refill`">{{
            t("admin.wafSettings.violationRate.refill")
          }}</Label>
          <Input
            :id="`${id}-refill`"
            v-model="form.refill"
            type="number"
            min="1"
            max="86400"
            step="1"
            required
            :disabled="disabled"
          />
        </div>
        <p class="text-sm text-muted-foreground sm:col-span-2">
          {{
            t("admin.wafSettings.violationRate.explanation", {
              capacity: form.capacity,
              refill: form.refill,
              trigger: Number(form.capacity) + 1,
            })
          }}
        </p>
      </div>
      <Button type="submit" :disabled="disabled || !valid">{{
        t("admin.wafSettings.violationRate.save")
      }}</Button>
    </form>
  </section>
</template>
