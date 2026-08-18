<script setup lang="ts">
import { useI18n } from "vue-i18n";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type {
  PanelProvider,
  PanelProviderDescriptor,
} from "@/lib/api/panel-sync-api";

defineProps<{
  inputId?: string;
  modelValue: PanelProvider;
  providers: PanelProviderDescriptor[];
  disabled?: boolean;
}>();
const emit = defineEmits<{ "update:modelValue": [value: PanelProvider] }>();
const { t } = useI18n();
</script>

<template>
  <Select
    :model-value="modelValue"
    :disabled="disabled"
    @update:model-value="emit('update:modelValue', $event as PanelProvider)"
  >
    <SelectTrigger :id="inputId" class="w-full">
      <SelectValue
        :placeholder="t('admin.panelSync.editor.providerPlaceholder')"
      />
    </SelectTrigger>
    <SelectContent>
      <SelectItem
        v-for="item in providers"
        :key="item.provider"
        :value="item.provider"
      >
        {{ item.name }}
      </SelectItem>
    </SelectContent>
  </Select>
</template>
