import { ref, type Ref } from "vue";
import type { ProviderField, TargetDialogState } from "./model";

export const useDDNSFieldState = ({
  selectedProvider,
  targetDialogState,
}: {
  selectedProvider: Ref<string>;
  targetDialogState: Ref<TargetDialogState>;
}) => {
  const fieldVisibility = ref<Record<string, boolean>>({});
  const targetFieldVisibility = ref<Record<string, boolean>>({});
  const fieldEditReady = ref<Record<string, boolean>>({});

  const toggleFieldVisibility = (key: string) => {
    fieldVisibility.value[key] = !fieldVisibility.value[key];
  };

  const getTargetFieldStateKey = (key: string) =>
    `${targetDialogState.value.provider}:${targetDialogState.value.id || "new"}:${key}`;

  const toggleTargetFieldVisibility = (key: string) => {
    const stateKey = getTargetFieldStateKey(key);
    targetFieldVisibility.value[stateKey] =
      !targetFieldVisibility.value[stateKey];
  };

  const isTargetFieldVisible = (key: string) => {
    return targetFieldVisibility.value[getTargetFieldStateKey(key)] === true;
  };

  const getFieldStateKey = (key: string) => `${selectedProvider.value}:${key}`;

  const getFieldDomId = (index: number) => `ddns-field-${index}`;

  const getFieldInputName = (index: number) => `ddns-input-${index}`;

  const enableFieldEditing = (key: string) => {
    fieldEditReady.value[getFieldStateKey(key)] = true;
  };

  const isFieldEditReady = (key: string) => {
    return fieldEditReady.value[getFieldStateKey(key)] === true;
  };

  const getFieldAutocomplete = (field: ProviderField) => {
    const normalizedKey = field.key.toLowerCase();
    if (
      field.type === "password" ||
      /access|account|auth|credential|email|key|login|secret|token|user/.test(
        normalizedKey,
      )
    ) {
      return "new-password";
    }

    return "off";
  };

  const resetFieldEditReady = () => {
    fieldEditReady.value = {};
  };

  const resetTargetFieldVisibility = () => {
    targetFieldVisibility.value = {};
  };

  const ensurePasswordFieldsVisible = (fields: ProviderField[]) => {
    for (const field of fields) {
      if (field.type === "password" && !(field.key in fieldVisibility.value)) {
        fieldVisibility.value[field.key] = true;
      }
    }
  };

  return {
    enableFieldEditing,
    ensurePasswordFieldsVisible,
    fieldVisibility,
    getFieldAutocomplete,
    getFieldDomId,
    getFieldInputName,
    isFieldEditReady,
    isTargetFieldVisible,
    resetFieldEditReady,
    resetTargetFieldVisibility,
    toggleFieldVisibility,
    toggleTargetFieldVisibility,
  };
};
