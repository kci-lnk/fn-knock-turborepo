import type { ComputedRef, Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  DDNSAPI,
  type DDNSNetworkInterfacePayload,
  type DDNSStatusPayload,
} from "@/lib/api";
import {
  extractCommonTargetConfig,
  type Provider,
  type ProviderField,
} from "./model";

export const useDDNSResourceLoading = ({
  applyStatus,
  currentProviderDef,
  ensurePasswordFieldsVisible,
  isInitialized,
  isPrimaryConfigDirty,
  networkInterfaces,
  providerConfig,
  providers,
  resetFieldEditReady,
  savedProviderConfig,
  selectedProvider,
}: {
  applyStatus: (
    status: DDNSStatusPayload,
    options?: { syncEnabled?: boolean; syncProvider?: boolean },
  ) => void;
  currentProviderDef: ComputedRef<Provider | null>;
  ensurePasswordFieldsVisible: (fields: ProviderField[]) => void;
  isInitialized: Ref<boolean>;
  isPrimaryConfigDirty: Readonly<Ref<boolean>>;
  networkInterfaces: Ref<DDNSNetworkInterfacePayload[]>;
  providerConfig: Ref<Record<string, string>>;
  providers: Ref<Provider[]>;
  resetFieldEditReady: () => void;
  savedProviderConfig: Ref<Record<string, string>>;
  selectedProvider: Ref<string>;
}) => {
  const { t } = useI18n();

  const { run: runLoadStatus } = useAsyncAction({
    onError: (error) => {
      console.error(
        "loadStatus:",
        extractErrorMessage(error, t("admin.ddns.loadStatusFailed")),
      );
    },
  });
  const { run: runLoadProviders } = useAsyncAction({
    onError: (error) => {
      console.error(
        "loadProviders:",
        extractErrorMessage(error, t("admin.ddns.loadProvidersFailed")),
      );
    },
  });
  const { run: runLoadNetworkInterfaces } = useAsyncAction({
    onError: (error) => {
      console.error(
        "loadNetworkInterfaces:",
        extractErrorMessage(error, t("admin.ddns.loadInterfacesFailed")),
      );
    },
  });
  const { run: runLoadConfig } = useAsyncAction({
    onError: (error) => {
      console.error(
        "loadConfig:",
        extractErrorMessage(error, t("admin.ddns.loadConfigFailed")),
      );
    },
  });
  const { isPending: isLoading, run: runInitialize } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.initFailed"), {
        description: extractErrorMessage(error, t("admin.ddns.initLoadFailed")),
      });
    },
  });

  const loadStatus = async () => {
    await runLoadStatus(async () => {
      const status = await DDNSAPI.getStatus();
      applyStatus(status, {
        syncProvider: !isInitialized.value || !isPrimaryConfigDirty.value,
      });
    });
  };

  const loadProviders = async () => {
    await runLoadProviders(async () => {
      const data = await DDNSAPI.getProviders();
      providers.value = data.map((provider) => ({
        ...provider,
        fields: provider.fields.map((field) => ({
          ...field,
          type: field.type as ProviderField["type"],
        })),
      }));
    });
  };

  const loadNetworkInterfaces = async () => {
    await runLoadNetworkInterfaces(async () => {
      networkInterfaces.value = await DDNSAPI.getNetworkInterfaces();
    });
  };

  const loadConfig = async () => {
    if (!selectedProvider.value) {
      providerConfig.value = {};
      savedProviderConfig.value = {};
      return;
    }

    await runLoadConfig(async () => {
      const config = await DDNSAPI.getConfig(selectedProvider.value);
      const providerDef = currentProviderDef.value;
      const merged = extractCommonTargetConfig(config);

      resetFieldEditReady();
      if (providerDef) {
        for (const field of providerDef.fields) {
          merged[field.key] = config[field.key] ?? "";
        }
        ensurePasswordFieldsVisible(providerDef.fields);
      }
      providerConfig.value = merged;
      savedProviderConfig.value = { ...merged };
    });
  };

  const initialize = async () => {
    const initialized = await runInitialize(async () => {
      await Promise.all([
        loadProviders(),
        loadStatus(),
        loadNetworkInterfaces(),
      ]);
      await loadConfig();
      return true;
    });
    isInitialized.value = true;
    return Boolean(initialized);
  };

  return {
    initialize,
    isLoading,
    loadConfig,
    loadStatus,
  };
};
