<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import {
  updateProxyValidationKey,
  type DDNSUpdateProxyDraft,
} from "./ddns-proxy-model";
const props = defineProps<{
  modelValue: DDNSUpdateProxyDraft;
  disabled: boolean;
}>();
const emit = defineEmits<{
  "update:modelValue": [value: DDNSUpdateProxyDraft];
}>();
const { t } = useI18n();
const error = computed(() => updateProxyValidationKey(props.modelValue));
const update = (patch: Partial<DDNSUpdateProxyDraft>) =>
  emit("update:modelValue", { ...props.modelValue, ...patch });
</script>

<template>
  <section
    class="min-w-0 space-y-3 rounded-lg border p-4"
    aria-labelledby="ddns-update-proxy-title"
  >
    <h3 id="ddns-update-proxy-title" class="text-sm font-medium">
      {{ t("admin.ddns.updateProxyTitle") }}
    </h3>
    <p class="text-xs leading-5 text-muted-foreground">
      {{ t("admin.ddns.updateProxyHint") }}
    </p>
    <div class="flex min-h-11 items-center justify-between gap-3">
      <Label for="ddns-update-proxy-enabled">{{
        t("admin.ddns.updateProxyEnabled")
      }}</Label>
      <Switch
        id="ddns-update-proxy-enabled"
        :model-value="modelValue.enabled"
        :disabled="disabled"
        @update:model-value="update({ enabled: $event })"
      />
    </div>
    <div class="space-y-2">
      <Label for="ddns-update-proxy-url">{{
        t("admin.ddns.updateProxyUrl")
      }}</Label>
      <Input
        id="ddns-update-proxy-url"
        :model-value="modelValue.url"
        :disabled="disabled"
        class="min-w-0"
        placeholder="http://127.0.0.1:7890"
        autocomplete="off"
        autocapitalize="none"
        :spellcheck="false"
        :aria-invalid="Boolean(error)"
        aria-describedby="ddns-update-proxy-error ddns-update-proxy-formats"
        @update:model-value="update({ url: String($event) })"
      />
      <p
        id="ddns-update-proxy-formats"
        class="break-words text-xs text-muted-foreground"
      >
        {{ t("admin.ddns.updateProxyFormats") }}
      </p>
    </div>
    <div class="grid min-w-0 gap-3 sm:grid-cols-2">
      <div class="min-w-0 space-y-2">
        <Label for="ddns-update-proxy-username">{{
          t("admin.ddns.updateProxyUsername")
        }}</Label>
        <Input
          id="ddns-update-proxy-username"
          :model-value="modelValue.username"
          :disabled="disabled"
          autocomplete="off"
          autocapitalize="none"
          :spellcheck="false"
          @update:model-value="update({ username: String($event) })"
        />
      </div>
      <div class="min-w-0 space-y-2">
        <Label for="ddns-update-proxy-password">{{
          t("admin.ddns.updateProxyPassword")
        }}</Label>
        <Input
          id="ddns-update-proxy-password"
          :model-value="modelValue.password"
          :disabled="disabled"
          type="password"
          autocomplete="new-password"
          :placeholder="
            modelValue.passwordConfigured && !modelValue.clearPassword
              ? t('admin.ddns.updateProxyPasswordSaved')
              : ''
          "
          @update:model-value="
            update({ password: String($event), clearPassword: false })
          "
        />
      </div>
    </div>
    <Button
      v-if="modelValue.passwordConfigured"
      type="button"
      variant="outline"
      class="min-h-11 w-full sm:w-auto"
      :disabled="disabled"
      :aria-pressed="modelValue.clearPassword"
      @click="
        update({ clearPassword: !modelValue.clearPassword, password: '' })
      "
    >
      {{
        t(
          modelValue.clearPassword
            ? "admin.ddns.updateProxyUndoClear"
            : "admin.ddns.updateProxyClearPassword",
        )
      }}
    </Button>
    <p
      v-if="error"
      id="ddns-update-proxy-error"
      role="alert"
      class="break-words text-sm text-destructive"
    >
      {{ t(error) }}
    </p>
    <slot />
  </section>
</template>
