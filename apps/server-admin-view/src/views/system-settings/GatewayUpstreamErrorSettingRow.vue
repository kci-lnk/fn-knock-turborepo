<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import type { GatewayUpstreamErrorDetail } from "@/types";

const props = defineProps<{
  disabled?: boolean;
  modelValue: GatewayUpstreamErrorDetail;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: GatewayUpstreamErrorDetail];
}>();

const selectDetail = (value: GatewayUpstreamErrorDetail) => {
  if (!props.disabled) emit("update:modelValue", value);
};

const { t } = useI18n();
</script>

<template>
  <div
    class="grid gap-3 p-6 sm:grid-cols-[minmax(0,1fr)_auto] sm:items-center sm:gap-4"
  >
    <div class="space-y-1 pr-6">
      <div class="text-base font-medium">
        {{ t("admin.gatewaySettings.upstreamErrorDetail") }}
      </div>
      <div class="text-sm text-muted-foreground">
        {{ t("admin.gatewaySettings.upstreamErrorDetailDescription") }}
      </div>
    </div>
    <div
      role="group"
      :aria-label="t('admin.gatewaySettings.upstreamErrorDetail')"
      class="grid w-full gap-1 rounded-md border bg-background p-1 sm:inline-flex sm:w-fit sm:gap-0"
    >
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'less' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'less'"
        @click="selectDetail('less')"
      >
        {{ t("admin.gatewaySettings.upstreamErrorDetailLess") }}
      </Button>
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'more' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'more'"
        @click="selectDetail('more')"
      >
        {{ t("admin.gatewaySettings.upstreamErrorDetailMore") }}
      </Button>
      <Button
        type="button"
        size="sm"
        :variant="modelValue === 'reset_connection' ? 'default' : 'ghost'"
        class="h-8 px-3"
        :disabled="disabled"
        :aria-pressed="modelValue === 'reset_connection'"
        @click="selectDetail('reset_connection')"
      >
        {{ t("admin.gatewaySettings.upstreamErrorDetailReset") }}
      </Button>
    </div>
  </div>
</template>
