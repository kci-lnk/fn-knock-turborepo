import { computed, type Ref } from "vue";
import { useDnsCredentialTransfer } from "@/composables/useDnsCredentialTransfer";
import { toast } from "@admin-shared/utils/toast";

type Translate = (
  key: string,
  params?: Record<string, string | number>,
) => string;

interface UseDDNSCredentialTransferHintOptions {
  enableFieldEditing: (key: string) => void;
  providerConfig: Ref<Record<string, string>>;
  selectedProvider: Ref<string>;
  translate: Translate;
}

export function useDDNSCredentialTransferHint({
  enableFieldEditing,
  providerConfig,
  selectedProvider,
  translate,
}: UseDDNSCredentialTransferHintOptions) {
  const {
    applySuggestion,
    isLoadingSource: isTransferSourceLoading,
    sourceScopeLabel: transferSourceScopeLabel,
    suggestion: credentialTransferSuggestion,
  } = useDnsCredentialTransfer({
    target: "ddns",
    providerId: selectedProvider,
    targetCredentials: providerConfig,
  });

  const credentialTransferDescription = computed(() => {
    const suggestion = credentialTransferSuggestion.value;
    if (!suggestion) return "";
    return translate("admin.ddns.credentialTransferDescription", {
      scope: transferSourceScopeLabel.value,
      bridge: suggestion.bridgeLabel,
      count: suggestion.fillableFields.length,
    });
  });

  const applyCredentialTransfer = () => {
    const result = applySuggestion();
    if (!result) return;
    for (const key of result.appliedKeys) enableFieldEditing(key);
    toast.success(
      translate("admin.ddns.credentialsApplied", {
        scope: transferSourceScopeLabel.value,
        count: result.count,
      }),
    );
  };

  return {
    applyCredentialTransfer,
    credentialTransferDescription,
    credentialTransferSuggestion,
    isTransferSourceLoading,
    transferSourceScopeLabel,
  };
}
