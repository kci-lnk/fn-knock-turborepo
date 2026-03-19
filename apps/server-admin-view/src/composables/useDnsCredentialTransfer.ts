import { computed, ref, watch, type Ref } from "vue";
import { AcmeAPI, DDNSAPI } from "@/lib/api";
import {
  buildDnsCredentialTransferSuggestion,
  resolveDnsCredentialBridge,
  type DnsCredentialTarget,
} from "@/lib/dns-credential-bridge";

type UseDnsCredentialTransferOptions = {
  target: DnsCredentialTarget;
  providerId: Readonly<Ref<string>>;
  targetCredentials: Ref<Record<string, string>>;
};

export const useDnsCredentialTransfer = ({
  target,
  providerId,
  targetCredentials,
}: UseDnsCredentialTransferOptions) => {
  const isLoadingSource = ref(false);
  const sourceCredentials = ref<Record<string, string>>({});
  let requestId = 0;

  const sourceScopeLabel = computed(() =>
    target === "acme" ? "DDNS" : "ACME",
  );

  const refreshSource = async () => {
    const currentProviderId = providerId.value.trim();
    const bridge = resolveDnsCredentialBridge(target, currentProviderId);
    const currentRequestId = ++requestId;

    sourceCredentials.value = {};

    if (!bridge) {
      isLoadingSource.value = false;
      return;
    }

    isLoadingSource.value = true;

    try {
      if (target === "acme") {
        const config = await DDNSAPI.getConfig(bridge.ddnsProvider);
        if (currentRequestId !== requestId) return;
        sourceCredentials.value = config || {};
        return;
      }

      const config = await AcmeAPI.getConfig();
      if (currentRequestId !== requestId) return;
      sourceCredentials.value =
        config?.dnsType === bridge.acmeDnsType ? config.credentials || {} : {};
    } catch (error) {
      if (currentRequestId !== requestId) return;
      console.error("useDnsCredentialTransfer:", error);
      sourceCredentials.value = {};
    } finally {
      if (currentRequestId === requestId) {
        isLoadingSource.value = false;
      }
    }
  };

  const suggestion = computed(() => {
    return buildDnsCredentialTransferSuggestion({
      target,
      providerId: providerId.value,
      sourceCredentials: sourceCredentials.value,
      targetCredentials: targetCredentials.value,
    });
  });

  const applySuggestion = () => {
    const currentSuggestion = suggestion.value;
    if (!currentSuggestion) return null;

    const nextCredentials = { ...targetCredentials.value };
    const appliedKeys: string[] = [];

    for (const field of currentSuggestion.fillableFields) {
      if ((nextCredentials[field.targetKey] || "").trim()) continue;
      nextCredentials[field.targetKey] = field.value;
      appliedKeys.push(field.targetKey);
    }

    if (appliedKeys.length === 0) return null;

    targetCredentials.value = nextCredentials;

    return {
      appliedKeys,
      count: appliedKeys.length,
    };
  };

  watch(providerId, () => {
    void refreshSource();
  }, { immediate: true });

  return {
    applySuggestion,
    isLoadingSource,
    refreshSource,
    sourceScopeLabel,
    suggestion,
  };
};
