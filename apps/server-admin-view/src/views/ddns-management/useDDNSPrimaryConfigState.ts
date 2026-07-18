import { computed, type Ref } from "vue";
import {
  findProviderDef,
  hasConfiguredProviderFields,
  isProviderConfigEqual,
  type Provider,
} from "./model";

interface UseDDNSPrimaryConfigStateOptions {
  confirmProviderChange: () => Promise<boolean>;
  loadConfig: () => Promise<unknown>;
  providerConfig: Ref<Record<string, string>>;
  providers: Ref<Provider[]>;
  savedProvider: Ref<string>;
  savedProviderConfig: Ref<Record<string, string>>;
  selectedProvider: Ref<string>;
}

export function useDDNSPrimaryConfigState({
  confirmProviderChange,
  loadConfig,
  providerConfig,
  providers,
  savedProvider,
  savedProviderConfig,
  selectedProvider,
}: UseDDNSPrimaryConfigStateOptions) {
  const currentProviderDef = computed(() =>
    findProviderDef(providers.value, selectedProvider.value),
  );
  const hasProviderConfig = computed(() =>
    hasConfiguredProviderFields(currentProviderDef.value, providerConfig.value),
  );
  const hasSavedProviderConfig = computed(() =>
    hasConfiguredProviderFields(
      currentProviderDef.value,
      savedProviderConfig.value,
    ),
  );
  const isPrimaryConfigValueDirty = computed(
    () =>
      Boolean(selectedProvider.value) &&
      !isProviderConfigEqual(providerConfig.value, savedProviderConfig.value),
  );
  const isPrimaryProviderDirty = computed(
    () => selectedProvider.value !== savedProvider.value,
  );
  const isPrimaryConfigDirty = computed(
    () => isPrimaryProviderDirty.value || isPrimaryConfigValueDirty.value,
  );

  async function onProviderChange(value: string) {
    if (!value || value === selectedProvider.value) return;
    if (isPrimaryConfigValueDirty.value && !(await confirmProviderChange())) {
      return;
    }
    selectedProvider.value = value;
    await loadConfig();
  }

  const setProviderConfigField = (key: string, value: string) => {
    providerConfig.value[key] = value;
  };

  return {
    currentProviderDef,
    hasProviderConfig,
    hasSavedProviderConfig,
    isPrimaryConfigDirty,
    isPrimaryConfigValueDirty,
    onProviderChange,
    setProviderConfigField,
  };
}
