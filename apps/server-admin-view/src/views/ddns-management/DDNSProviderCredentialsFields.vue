<script setup lang="ts">
import { Eye, EyeOff } from "lucide-vue-next";
import { useI18n } from "vue-i18n";
import CredentialTransferHint from "@/components/CredentialTransferHint.vue";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import type { DnsCredentialTransferSuggestion } from "@/lib/dns-credential-bridge";
import type { Provider, ProviderField } from "./model";

defineProps<{
  credentialTransferDescription: string;
  credentialTransferSuggestion: DnsCredentialTransferSuggestion | null;
  enableFieldEditing: (key: string) => void;
  fieldVisibility: Record<string, boolean>;
  getFieldAutocomplete: (field: ProviderField) => string;
  getFieldDescription: (field: ProviderField) => string;
  getFieldDomId: (index: number) => string;
  getFieldInputName: (index: number) => string;
  isFieldEditReady: (key: string) => boolean;
  isTransferSourceLoading: boolean;
  providerConfig: Record<string, string>;
  providerDef: Provider | null;
  setFieldValue: (key: string, value: string) => void;
  toggleFieldVisibility: (key: string) => void;
  transferSourceScopeLabel: string;
}>();

const emit = defineEmits<{
  applyCredentialTransfer: [];
}>();

const { t } = useI18n();
</script>

<template>
  <template v-if="providerDef">
    <div
      v-if="credentialTransferSuggestion"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label class="text-sm font-medium">
          {{ t("admin.ddns.credentialReuse") }}
        </Label>
        <p class="text-xs text-muted-foreground hidden sm:block pr-4">
          {{ t("admin.ddns.credentialReuseHint") }}
        </p>
      </div>

      <div class="w-full max-w-2xl space-y-2">
        <CredentialTransferHint
          :action-label="
            t('admin.ddns.credentialFillAction', {
              scope: transferSourceScopeLabel,
            })
          "
          :description="credentialTransferDescription"
          :fields="
            credentialTransferSuggestion.fillableFields.map(
              (field) => field.targetKey,
            )
          "
          :loading="isTransferSourceLoading"
          :source-label="`${transferSourceScopeLabel} · ${credentialTransferSuggestion.bridgeLabel}`"
          @apply="emit('applyCredentialTransfer')"
        />

        <p class="text-[11px] text-muted-foreground sm:hidden mt-1.5">
          {{ t("admin.ddns.credentialReuseHint") }}
        </p>
      </div>
    </div>

    <div
      v-for="(field, index) in providerDef.fields"
      :key="field.key"
      class="p-4 sm:p-6 grid gap-2 sm:grid-cols-[200px_1fr] md:grid-cols-[240px_1fr] items-start transition-colors hover:bg-muted/10"
    >
      <div class="space-y-1 mt-1.5">
        <Label
          :for="getFieldDomId(index)"
          class="text-sm font-medium flex items-center gap-1"
        >
          {{ field.label }}
          <span v-if="field.required !== false" class="text-destructive">
            *
          </span>
        </Label>
        <p
          v-if="getFieldDescription(field)"
          class="text-xs text-muted-foreground leading-relaxed hidden sm:block pr-4"
        >
          {{ getFieldDescription(field) }}
        </p>
      </div>

      <div class="w-full max-w-md space-y-2">
        <Select
          v-if="field.type === 'select' && field.options"
          :modelValue="
            providerConfig[field.key] ||
            (field.options && field.options[0]?.value) ||
            ''
          "
          @update:modelValue="
            (val: any) => setFieldValue(field.key, String(val ?? ''))
          "
        >
          <SelectTrigger class="w-full" :id="getFieldDomId(index)">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="opt in field.options"
              :key="opt.value"
              :value="opt.value"
            >
              {{ opt.label }}
            </SelectItem>
          </SelectContent>
        </Select>

        <div v-else-if="field.type === 'password'" class="relative">
          <Input
            :id="getFieldDomId(index)"
            :name="getFieldInputName(index)"
            :type="fieldVisibility[field.key] ? 'text' : 'password'"
            :placeholder="field.placeholder"
            :autocomplete="getFieldAutocomplete(field)"
            :readonly="!isFieldEditReady(field.key)"
            :model-value="providerConfig[field.key] || ''"
            class="pr-10"
            @update:model-value="
              (value: string | number) => setFieldValue(field.key, String(value))
            "
            @focus="enableFieldEditing(field.key)"
            @pointerdown="enableFieldEditing(field.key)"
          />
          <button
            type="button"
            class="absolute right-3 top-1/2 -translate-y-1/2 text-muted-foreground hover:text-foreground transition-colors"
            @click="toggleFieldVisibility(field.key)"
          >
            <component
              :is="fieldVisibility[field.key] ? EyeOff : Eye"
              class="h-4 w-4"
            />
          </button>
        </div>

        <Input
          v-else
          :id="getFieldDomId(index)"
          :name="getFieldInputName(index)"
          :type="field.type"
          :placeholder="field.placeholder"
          :autocomplete="getFieldAutocomplete(field)"
          :readonly="!isFieldEditReady(field.key)"
          :model-value="providerConfig[field.key] || ''"
          @update:model-value="
            (value: string | number) => setFieldValue(field.key, String(value))
          "
          @focus="enableFieldEditing(field.key)"
          @pointerdown="enableFieldEditing(field.key)"
        />

        <p
          v-if="getFieldDescription(field)"
          class="text-[11px] text-muted-foreground sm:hidden mt-1.5"
        >
          {{ getFieldDescription(field) }}
        </p>
      </div>
    </div>
  </template>
</template>
