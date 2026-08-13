<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { Eye, EyeOff } from "lucide-vue-next";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { Provider, ProviderField, TargetDialogState } from "./model";

defineProps<{
  formatDomainField: () => void;
  getFieldAutocomplete: (field: ProviderField) => string;
  getFieldDescription: (field: ProviderField) => string;
  isFieldVisible: (key: string) => boolean;
  providerDef: Provider;
  state: TargetDialogState;
  toggleFieldVisibility: (key: string) => void;
}>();
const { t } = useI18n();
</script>

<template>
  <div
    v-for="field in providerDef.fields"
    :key="`target-${field.key}`"
    class="grid items-start gap-2 p-4 transition-colors hover:bg-muted/10 sm:grid-cols-[180px_1fr] sm:p-5 md:grid-cols-[220px_1fr]"
  >
    <div class="mt-1.5 space-y-1">
      <Label
        :for="`ddns-target-field-${field.key}`"
        class="flex items-center gap-1 text-sm font-medium"
      >
        {{ field.label }}
        <span v-if="field.required !== false" class="text-destructive">*</span>
      </Label>
      <p
        v-if="getFieldDescription(field)"
        class="hidden pr-4 text-xs text-muted-foreground sm:block"
      >
        {{ getFieldDescription(field) }}
      </p>
    </div>

    <div class="w-full max-w-md space-y-2">
      <Select
        v-if="field.type === 'select' && field.options"
        :model-value="state.config[field.key] || field.options[0]?.value || ''"
        @update:model-value="state.config[field.key] = String($event ?? '')"
      >
        <SelectTrigger :id="`ddns-target-field-${field.key}`">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          <SelectItem
            v-for="option in field.options"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </SelectItem>
        </SelectContent>
      </Select>

      <div v-else-if="field.type === 'password'" class="relative">
        <Input
          :id="`ddns-target-field-${field.key}`"
          v-model="state.config[field.key]"
          :type="isFieldVisible(field.key) ? 'text' : 'password'"
          :placeholder="field.placeholder"
          :autocomplete="getFieldAutocomplete(field)"
          class="pr-10"
        />
        <button
          type="button"
          :aria-label="
            isFieldVisible(field.key)
              ? t('common.hideSecret')
              : t('common.showSecret')
          "
          class="absolute top-1/2 right-3 -translate-y-1/2 text-muted-foreground transition-colors hover:text-foreground"
          @click="toggleFieldVisibility(field.key)"
        >
          <component
            :is="isFieldVisible(field.key) ? EyeOff : Eye"
            class="h-4 w-4"
          />
        </button>
      </div>

      <Input
        v-else
        :id="`ddns-target-field-${field.key}`"
        v-model="state.config[field.key]"
        :type="field.type"
        :placeholder="field.placeholder"
        :autocomplete="getFieldAutocomplete(field)"
        @blur="field.key === 'domain' && formatDomainField()"
      />

      <p
        v-if="getFieldDescription(field)"
        class="text-[11px] text-muted-foreground sm:hidden"
      >
        {{ getFieldDescription(field) }}
      </p>
    </div>
  </div>
</template>
