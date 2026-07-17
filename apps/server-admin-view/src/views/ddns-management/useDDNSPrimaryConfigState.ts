import { computed, type Ref } from "vue";
import {
  findProviderDef,
  hasConfiguredProviderFields,
  isProviderConfigEqual,
  type Provider,
} from "./model";

interface UseDDNSPrimaryConfigStateOptions {
  loadConfig: () => Promise<unknown>;
  providerConfig: Ref<Record<string, string>>;
  providers: Ref<Provider[]>;
  savedProvider: Ref<string>;
  savedProviderConfig: Ref<Record<string, string>>;
  selectedProvider: Ref<string>;
  translate: (key: string) => string;
}

export function useDDNSPrimaryConfigState({
  loadConfig,
  providerConfig,
  providers,
  savedProvider,
  savedProviderConfig,
  selectedProvider,
  translate,
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
    if (
      isPrimaryConfigValueDirty.value &&
      !window.confirm(translate("admin.ddns.unsavedSwitchProviderConfirm"))
    ) {
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
