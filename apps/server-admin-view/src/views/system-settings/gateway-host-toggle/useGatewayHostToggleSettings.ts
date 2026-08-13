import { computed, onMounted, reactive, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useAccessEntryPort } from "@/composables/useAccessEntryPort";
import {
  resolveExplicitPublicAccessEntryPort,
  shouldOmitPublicAccessEntryPort,
} from "@/lib/reverse-proxy-submode";
import { useConfigStore } from "@/store/config";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { useDelayedLoading } from "@admin-shared/composables/useDelayedLoading";
import { toast } from "@admin-shared/utils/toast";
import type {
  GatewayHostToggleDetails,
  GatewayHostToggleItem,
  GatewayHostToggleOptions,
} from "./gatewayHostToggleTypes";

const cloneItem = (item: GatewayHostToggleItem): GatewayHostToggleItem => ({
  ...item,
});

export const useGatewayHostToggleSettings = (
  options: GatewayHostToggleOptions,
) => {
  const { t } = useI18n();
  const configStore = useConfigStore();
  const { accessEntryPort, loadAccessEntryPort } = useAccessEntryPort();
  const details = ref<GatewayHostToggleDetails | null>(null);
  const formItems = ref<GatewayHostToggleItem[]>([]);
  const loadError = ref("");
  const message = (key: string) => t(`${options.messageKeyPrefix}.${key}`);
  const getToggleValue = (item: GatewayHostToggleItem) =>
    item[options.toggleField] === true;

  const applyDetails = (value: GatewayHostToggleDetails) => {
    details.value = {
      config: { disabled_hosts: [...value.config.disabled_hosts] },
      availability: { ...value.availability },
      items: value.items.map(cloneItem),
      summary: { ...value.summary },
    };
    formItems.value = value.items.map(cloneItem);
    if (configStore.config) {
      configStore.config = {
        ...configStore.config,
        [options.configStoreKey]: {
          disabled_hosts: [...value.config.disabled_hosts],
        },
      } as typeof configStore.config;
    }
  };

  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      loadError.value = extractErrorMessage(error, message("loadDescription"));
    },
  });
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(message("saveFailed"), {
        description: extractErrorMessage(error, message("saveDescription")),
      });
    },
  });
  const showLoadingSkeleton = useDelayedLoading(isLoading);
  const isAvailable = computed(
    () => details.value?.availability.available === true,
  );
  const isDirty = computed(() => {
    const summarize = (items: GatewayHostToggleItem[]) =>
      items.map((item) => ({
        host: item.host,
        value: getToggleValue(item),
      }));
    return (
      JSON.stringify(summarize(formItems.value)) !==
      JSON.stringify(summarize(details.value?.items ?? []))
    );
  });
  const saveBlockedReason = computed(() =>
    isAvailable.value
      ? ""
      : details.value?.availability.reason || message("unavailable"),
  );
  const disabledHosts = computed(() =>
    formItems.value
      .filter((item) => !getToggleValue(item))
      .map((item) => item.host),
  );
  const explicitAccessEntryPort = computed(() =>
    resolveExplicitPublicAccessEntryPort(configStore.config),
  );
  const displayAccessEntryPort = computed(() =>
    explicitAccessEntryPort.value
      ? String(explicitAccessEntryPort.value)
      : accessEntryPort.value.trim() || "7999",
  );
  const shouldOmitAccessEntryPort = computed(() => {
    if (
      shouldOmitPublicAccessEntryPort(configStore.config) &&
      !explicitAccessEntryPort.value
    ) {
      return true;
    }
    const port = Number.parseInt(displayAccessEntryPort.value, 10);
    return port === 80 || port === 443;
  });
  const formatHostWithAccessEntryPort = (host: string) =>
    shouldOmitAccessEntryPort.value
      ? host
      : `${host}:${displayAccessEntryPort.value}`;

  const fetchHostToggleDetails = async () => {
    await runLoad(async () => {
      const value = await options.fetchDetails();
      loadError.value = "";
      applyDetails(value);
    });
  };
  const resetForm = () => {
    if (details.value) formItems.value = details.value.items.map(cloneItem);
  };
  const updateHostToggle = (host: string, nextValue: boolean) => {
    if (isSaving.value || !isAvailable.value) return;
    formItems.value = formItems.value.map((item) =>
      item.host === host
        ? { ...item, [options.toggleField]: nextValue }
        : item,
    );
  };
  const saveSettings = async () => {
    if (saveBlockedReason.value) {
      toast.error(message("saveBlockedTitle"), {
        description: saveBlockedReason.value,
      });
      return;
    }
    await runSave(
      () => options.saveDetails({ disabled_hosts: disabledHosts.value }),
      {
        onSuccess: (value) => {
          applyDetails(value);
          toast.success(message("updated"));
        },
      },
    );
  };

  onMounted(() => {
    void fetchHostToggleDetails();
    void loadAccessEntryPort();
  });

  return reactive({
    details,
    formItems,
    formatHostWithAccessEntryPort,
    getToggleValue,
    isAvailable,
    isDirty,
    isLoading,
    isSaving,
    loadError,
    message,
    resetForm,
    saveBlockedReason,
    saveSettings,
    showLoadingSkeleton,
    updateHostToggle,
  });
};

export type GatewayHostToggleSettingsModel = ReturnType<
  typeof useGatewayHostToggleSettings
>;
