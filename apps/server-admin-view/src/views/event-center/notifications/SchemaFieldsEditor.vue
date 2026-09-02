<script setup lang="ts">
import { computed, useId } from "vue";
import { useI18n } from "vue-i18n";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { Textarea } from "@/components/ui/textarea";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type {
  NotificationHeaderEntry,
  NotificationSchemaField,
} from "../../../types";
import WebhookHeadersEditor from "./WebhookHeadersEditor.vue";
import { coerceWebhookHeaderEntries } from "./webhook-headers";

const a11yId = useId();

const { t } = useI18n();

const props = withDefaults(
  defineProps<{
    fields: NotificationSchemaField[];
    modelValue: Record<string, unknown>;
    configuredSensitiveFields?: string[];
    revealSensitiveValues?: boolean;
  }>(),
  {
    configuredSensitiveFields: () => [],
    revealSensitiveValues: false,
  },
);

const emit = defineEmits<{
  "update:modelValue": [value: Record<string, unknown>];
}>();

const configuredSensitiveFieldSet = computed(
  () => new Set(props.configuredSensitiveFields),
);

const getFieldDomId = (field: NotificationSchemaField) =>
  `${a11yId}-schemafieldseditor-${field.key}`;

const updateField = (key: string, value: unknown) => {
  emit("update:modelValue", {
    ...props.modelValue,
    [key]: value,
  });
};

const readFieldValue = (field: NotificationSchemaField) => {
  const value = props.modelValue[field.key];
  if (value === undefined || value === null) {
    if (field.type === "boolean") {
      return Boolean(field.default_value ?? false);
    }
    return field.default_value ?? "";
  }
  return value;
};

const readHeaderFieldValue = (
  field: NotificationSchemaField,
): NotificationHeaderEntry[] =>
  coerceWebhookHeaderEntries(readFieldValue(field));

const resolvePlaceholder = (field: NotificationSchemaField) => {
  if (
    field.sensitive &&
    configuredSensitiveFieldSet.value.has(field.key) &&
    !props.modelValue[field.key]
  ) {
    return t("admin.notifications.schema.sensitiveConfigured");
  }
  return field.placeholder || "";
};
</script>

<template>
  <div class="space-y-4">
    <div
      v-for="field in fields"
      :key="field.key"
      class="grid gap-2 rounded-md border border-border/60 p-3"
    >
      <div class="space-y-1">
        <Label
          :for="field.type === 'headers' ? undefined : getFieldDomId(field)"
          class="text-sm font-medium"
        >
          {{ field.label }}
          <span v-if="field.required" class="text-destructive">*</span>
        </Label>
        <p v-if="field.description" class="text-xs text-muted-foreground">
          {{ field.description }}
        </p>
      </div>

      <Input
        v-if="field.type === 'string'"
        :id="getFieldDomId(field)"
        :type="
          field.sensitive && !props.revealSensitiveValues ? 'password' : 'text'
        "
        :model-value="String(readFieldValue(field) ?? '')"
        :placeholder="resolvePlaceholder(field)"
        @update:model-value="(value) => updateField(field.key, value)"
      />

      <Input
        v-else-if="field.type === 'number'"
        :id="getFieldDomId(field)"
        type="number"
        :min="field.min"
        :max="field.max"
        :model-value="String(readFieldValue(field) ?? '')"
        :placeholder="resolvePlaceholder(field)"
        @update:model-value="(value) => updateField(field.key, value)"
      />

      <Select
        v-else-if="field.type === 'select'"
        :model-value="String(readFieldValue(field) ?? '')"
        @update:model-value="(value) => updateField(field.key, value)"
      >
        <SelectTrigger :id="getFieldDomId(field)">
          <SelectValue
            :placeholder="resolvePlaceholder(field) || field.label"
          />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in field.options || []"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div
        v-else-if="field.type === 'boolean'"
        class="flex items-center justify-between rounded-md border border-border/60 px-3 py-2"
      >
        <div class="text-sm text-muted-foreground">
          {{
            Boolean(readFieldValue(field))
              ? t("admin.notifications.schema.enabled")
              : t("admin.notifications.schema.disabled")
          }}
        </div>
        <Switch
          :id="getFieldDomId(field)"
          :model-value="Boolean(readFieldValue(field))"
          :aria-label="field.label"
          @update:model-value="(value) => updateField(field.key, value)"
        />
      </div>

      <WebhookHeadersEditor
        v-else-if="field.type === 'headers'"
        :model-value="readHeaderFieldValue(field)"
        :constraints="field.constraints"
        @update:model-value="(value) => updateField(field.key, value)"
      />

      <Textarea
        v-else
        :id="getFieldDomId(field)"
        class="min-h-[110px] font-mono text-xs"
        :model-value="String(readFieldValue(field) ?? '')"
        :placeholder="resolvePlaceholder(field)"
        @update:model-value="(value) => updateField(field.key, value)"
      />
    </div>
  </div>
</template>
