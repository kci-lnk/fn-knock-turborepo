<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { normalizeUpdateProxyMode } from "./ddns-proxy-model";
const props = defineProps<{
  id: string;
  modelValue?: string;
  defaultEnabled: boolean;
  configured: boolean;
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: string] }>();
const { t } = useI18n();
const mode = computed(() => normalizeUpdateProxyMode(props.modelValue));
const label = computed(() =>
  mode.value === "inherit"
    ? t("admin.ddns.updateProxyInherit", {
        mode: t(
          props.defaultEnabled
            ? "admin.ddns.updateProxyUse"
            : "admin.ddns.updateProxyDirect",
        ),
      })
    : t(
        mode.value === "proxy"
          ? "admin.ddns.updateProxyUse"
          : "admin.ddns.updateProxyDirect",
      ),
);
</script>
<template>
  <div class="min-w-0 space-y-2 p-4 sm:p-5">
    <Label :for="id">{{ t("admin.ddns.updateProxyMode") }}</Label>
    <Select
      :model-value="mode"
      :disabled="disabled"
      @update:model-value="emit('update:modelValue', String($event))"
    >
      <SelectTrigger :id="id" class="min-h-11 w-full"
        ><SelectValue>{{ label }}</SelectValue></SelectTrigger
      >
      <SelectContent>
        <SelectItem value="inherit">{{
          t("admin.ddns.updateProxyInherit", {
            mode: t(
              defaultEnabled
                ? "admin.ddns.updateProxyUse"
                : "admin.ddns.updateProxyDirect",
            ),
          })
        }}</SelectItem>
        <SelectItem value="direct">{{
          t("admin.ddns.updateProxyDirect")
        }}</SelectItem>
        <SelectItem value="proxy">{{
          t("admin.ddns.updateProxyUse")
        }}</SelectItem>
      </SelectContent>
    </Select>
    <p
      v-if="
        !configured &&
        (mode === 'proxy' || (mode === 'inherit' && defaultEnabled))
      "
      role="alert"
      class="text-sm text-destructive"
    >
      {{ t("admin.ddns.updateProxyMissing") }}
    </p>
    <p class="text-xs leading-5 text-muted-foreground">
      {{ t("admin.ddns.updateProxyModeHint") }}
    </p>
  </div>
</template>
