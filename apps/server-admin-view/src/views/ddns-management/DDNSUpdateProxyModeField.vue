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
  layout?: "primary" | "target";
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
  <div
    class="grid min-w-0 items-start gap-2 p-4 transition-colors hover:bg-muted/10"
    :class="
      layout === 'target'
        ? 'sm:grid-cols-[180px_minmax(0,1fr)] sm:p-5 md:grid-cols-[220px_minmax(0,1fr)]'
        : 'sm:grid-cols-[200px_minmax(0,1fr)] sm:p-6 md:grid-cols-[240px_minmax(0,1fr)]'
    "
  >
    <div class="mt-1.5 min-w-0 space-y-1">
      <Label :for="id" class="text-sm font-medium">{{
        t("admin.ddns.updateProxyMode")
      }}</Label>
      <p
        :id="`${id}-hint`"
        class="break-words text-xs leading-relaxed text-muted-foreground sm:pr-4"
      >
        {{ t("admin.ddns.updateProxyModeHint") }}
      </p>
    </div>
    <div class="min-w-0 w-full max-w-md space-y-2">
      <Select
        :model-value="mode"
        :disabled="disabled"
        @update:model-value="emit('update:modelValue', String($event))"
      >
        <SelectTrigger
          :id="id"
          :aria-describedby="`${id}-hint`"
          class="w-full max-sm:min-h-11"
        >
          <SelectValue>{{ label }}</SelectValue>
        </SelectTrigger>
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
        class="break-words text-sm text-destructive"
      >
        {{ t("admin.ddns.updateProxyMissing") }}
      </p>
    </div>
  </div>
</template>
