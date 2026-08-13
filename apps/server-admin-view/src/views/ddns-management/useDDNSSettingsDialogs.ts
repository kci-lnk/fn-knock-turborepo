import { ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import {
  DDNSAPI,
  type DDNSHttpTransport,
  type DDNSPublicCheckSourcesPayload,
  type DDNSPublicCheckTestResultPayload,
  type DDNSPublicDnsProvider,
} from "@/lib/api/ddns";
import {
  DEFAULT_DDNS_HTTP_TRANSPORT,
  DEFAULT_DDNS_PUBLIC_DNS_PROVIDER,
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
  NETWORK_INTERFACE_KEY,
  normalizeDDNSHttpTransport,
  normalizeDDNSPublicDnsProvider,
  normalizeNetworkInterface,
  normalizePublicCheckSources,
  normalizeUpdateIntervalMinutes,
  parseUpdateIntervalDraft,
} from "./model";

export const useDDNSSettingsDialogs = ({
  defaultPublicCheckSources,
  httpTransport,
  providerConfig,
  publicCheckSources,
  publicDnsProvider,
  updateIntervalMinutes,
}: {
  defaultPublicCheckSources: Ref<DDNSPublicCheckSourcesPayload>;
  httpTransport: Ref<DDNSHttpTransport>;
  providerConfig: Ref<Record<string, string>>;
  publicCheckSources: Ref<DDNSPublicCheckSourcesPayload>;
  publicDnsProvider: Ref<DDNSPublicDnsProvider>;
  updateIntervalMinutes: Ref<number>;
}) => {
  const { t } = useI18n();
  const updateIntervalDraft = ref(String(DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES));
  const publicCheckDraft = ref<DDNSPublicCheckSourcesPayload>(
    normalizePublicCheckSources(undefined),
  );
  const httpTransportDraft = ref<DDNSHttpTransport>(
    DEFAULT_DDNS_HTTP_TRANSPORT,
  );
  const publicDnsProviderDraft = ref<DDNSPublicDnsProvider>(
    DEFAULT_DDNS_PUBLIC_DNS_PROVIDER,
  );
  const publicCheckTestResults = ref<DDNSPublicCheckTestResultPayload[]>([]);
  const showUpdateIntervalDialog = ref(false);
  const showPublicCheckDialog = ref(false);

  const { isPending: isSavingUpdateInterval, run: runSaveUpdateInterval } =
    useAsyncAction({
      onError: (error) => {
        toast.error(t("admin.ddns.saveIntervalFailed"), {
          description: extractErrorMessage(
            error,
            t("admin.ddns.saveIntervalFailed"),
          ),
        });
      },
    });

  const {
    isPending: isSavingPublicCheckSources,
    run: runSavePublicCheckSources,
  } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.savePublicCheckFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.savePublicCheckFailed"),
        ),
      });
    },
  });

  const {
    isPending: isTestingPublicCheckSources,
    run: runTestPublicCheckSources,
  } = useAsyncAction({
    onError: (error) => {
      toast.error(t("admin.ddns.testPublicCheckSourcesFailed"), {
        description: extractErrorMessage(
          error,
          t("admin.ddns.testPublicCheckSourcesFailed"),
        ),
      });
    },
  });

  const openUpdateIntervalDialog = () => {
    updateIntervalDraft.value = String(updateIntervalMinutes.value);
    showUpdateIntervalDialog.value = true;
  };

  const saveUpdateInterval = async () => {
    const next = parseUpdateIntervalDraft(updateIntervalDraft.value);
    if (next === null) {
      toast.error(t("admin.ddns.intervalInvalid"), {
        description: t("admin.ddns.intervalInvalidDescription", {
          min: MIN_DDNS_UPDATE_INTERVAL_MINUTES,
          max: MAX_DDNS_UPDATE_INTERVAL_MINUTES,
        }),
      });
      return;
    }

    await runSaveUpdateInterval(
      () => DDNSAPI.saveSettings({ updateIntervalMinutes: next }),
      {
        onSuccess: (settings) => {
          updateIntervalMinutes.value = normalizeUpdateIntervalMinutes(
            settings.updateIntervalMinutes,
          );
          updateIntervalDraft.value = String(updateIntervalMinutes.value);
          showUpdateIntervalDialog.value = false;
          toast.success(t("admin.ddns.intervalSaved"));
        },
      },
    );
  };

  const openPublicCheckDialog = () => {
    publicCheckDraft.value = normalizePublicCheckSources(
      publicCheckSources.value,
    );
    httpTransportDraft.value = httpTransport.value;
    publicDnsProviderDraft.value = publicDnsProvider.value;
    publicCheckTestResults.value = [];
    showPublicCheckDialog.value = true;
  };

  const restorePublicCheckDefaults = () => {
    publicCheckDraft.value = normalizePublicCheckSources(
      defaultPublicCheckSources.value,
    );
    publicCheckTestResults.value = [];
  };

  const savePublicCheckSources = async (
    nextSources: DDNSPublicCheckSourcesPayload,
    nextHttpTransport: DDNSHttpTransport,
    nextPublicDnsProvider: DDNSPublicDnsProvider,
  ) => {
    await runSavePublicCheckSources(
      () =>
        DDNSAPI.saveSettings({
          publicCheckSources: normalizePublicCheckSources(nextSources),
          httpTransport: normalizeDDNSHttpTransport(nextHttpTransport),
          publicDnsProvider: normalizeDDNSPublicDnsProvider(
            nextPublicDnsProvider,
          ),
        }),
      {
        onSuccess: (settings) => {
          defaultPublicCheckSources.value = normalizePublicCheckSources(
            settings.defaultPublicCheckSources,
          );
          publicCheckSources.value = normalizePublicCheckSources(
            settings.publicCheckSources,
            defaultPublicCheckSources.value,
          );
          publicCheckDraft.value = normalizePublicCheckSources(
            settings.publicCheckSources,
            defaultPublicCheckSources.value,
          );
          httpTransport.value = normalizeDDNSHttpTransport(
            settings.httpTransport,
          );
          httpTransportDraft.value = httpTransport.value;
          publicDnsProvider.value = normalizeDDNSPublicDnsProvider(
            settings.publicDnsProvider,
          );
          publicDnsProviderDraft.value = publicDnsProvider.value;
          publicCheckTestResults.value = [];
          showPublicCheckDialog.value = false;
          toast.success(t("admin.ddns.publicCheckSaved"));
        },
      },
    );
  };

  const testPublicCheckSources = async (
    nextSources: DDNSPublicCheckSourcesPayload,
    nextHttpTransport: DDNSHttpTransport,
    nextPublicDnsProvider: DDNSPublicDnsProvider,
  ) => {
    const sources = normalizePublicCheckSources(
      nextSources,
      defaultPublicCheckSources.value,
    );
    if (sources.ipv4.length === 0 && sources.ipv6.length === 0) {
      publicCheckTestResults.value = [];
      toast.error(t("admin.ddns.publicCheckNoTestSourcesConfigured"));
      return;
    }

    await runTestPublicCheckSources(
      () =>
        DDNSAPI.testPublicCheckSources(sources, {
          httpTransport: normalizeDDNSHttpTransport(nextHttpTransport),
          publicDnsProvider: normalizeDDNSPublicDnsProvider(
            nextPublicDnsProvider,
          ),
          networkInterface: normalizeNetworkInterface(
            providerConfig.value[NETWORK_INTERFACE_KEY],
          ),
        }),
      {
        onSuccess: (payload) => {
          publicCheckTestResults.value = payload.results || [];
          if (publicCheckTestResults.value.length === 0) {
            toast.error(t("admin.ddns.publicCheckNoTestSourcesConfigured"));
            return;
          }
          const hasFailures = publicCheckTestResults.value.some(
            (item) => !item.success,
          );
          if (hasFailures) {
            toast.error(t("admin.ddns.publicCheckTestCompletedWithErrors"));
          } else {
            toast.success(t("admin.ddns.publicCheckTestCompleted"));
          }
        },
      },
    );
  };

  return {
    httpTransportDraft,
    isSavingPublicCheckSources,
    isSavingUpdateInterval,
    isTestingPublicCheckSources,
    openPublicCheckDialog,
    openUpdateIntervalDialog,
    publicCheckDraft,
    publicCheckTestResults,
    publicDnsProviderDraft,
    restorePublicCheckDefaults,
    savePublicCheckSources,
    saveUpdateInterval,
    showPublicCheckDialog,
    showUpdateIntervalDialog,
    testPublicCheckSources,
    updateIntervalDraft,
  };
};
