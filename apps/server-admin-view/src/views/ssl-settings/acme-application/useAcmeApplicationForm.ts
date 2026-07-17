import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type {
  AcmeApplicationPayload,
  AcmeApplicationRecord,
  AcmeDnsProvider,
} from "@/lib/api";
import { useDnsCredentialTransfer } from "@/composables/useDnsCredentialTransfer";
import { toast } from "@admin-shared/utils/toast";
import {
  buildAcmeCredentialsPayload,
  getProviderCredentialFields,
  getProviderGroupKey,
  getSatisfiedCredentialScheme,
  normalizeProviderCredentials,
  type DnsCredentialScheme,
} from "./acmeApplicationModel";

export type AcmeApplicationDialogProps = {
  open: boolean;
  mode: "create" | "edit";
  initialValue?: AcmeApplicationRecord | null;
  dnsProviders: AcmeDnsProvider[];
  pending?: boolean;
};

export type AcmeApplicationDialogEmit = {
  (event: "update:open", value: boolean): void;
  (event: "submit", payload: AcmeApplicationPayload): void;
};

export const useAcmeApplicationForm = (
  props: Readonly<AcmeApplicationDialogProps>,
  emit: AcmeApplicationDialogEmit,
) => {
  const { locale, t } = useI18n();
  const name = ref("");
  const domains = ref<string[]>([]);
  const dnsType = ref("");
  const credentials = ref<Record<string, string>>({});
  const renewEnabled = ref(true);
  const isCredentialsVisible = ref(false);
  const credentialEditReady = ref<Record<string, boolean>>({});

  const activeProvider = computed(() => {
    return (
      props.dnsProviders.find(
        (provider) => provider.dnsType === dnsType.value,
      ) || null
    );
  });
  const activeDnsType = computed(() => dnsType.value.trim());
  const activeCredentialSchemes = computed<DnsCredentialScheme[]>(
    () => activeProvider.value?.credentialSchemes || [],
  );
  const activeCredentialFields = computed(() =>
    getProviderCredentialFields(activeProvider.value),
  );
  const hasMultipleCredentialSchemes = computed(
    () => activeCredentialSchemes.value.length > 1,
  );
  const matchedCredentialScheme = computed(() =>
    getSatisfiedCredentialScheme(activeProvider.value, credentials.value),
  );
  const filledCredentialCount = computed(() => {
    return activeCredentialFields.value.filter(
      (field) => (credentials.value[field.key] || "").trim().length > 0,
    ).length;
  });

  const credentialSummary = computed(() => {
    if (!activeCredentialFields.value.length) {
      return t("admin.acmeApplicationDialog.noExtraCredentials");
    }
    if (matchedCredentialScheme.value) {
      return t("admin.acmeApplicationDialog.schemeSatisfied", {
        label: matchedCredentialScheme.value.label,
      });
    }
    if (!filledCredentialCount.value) {
      return hasMultipleCredentialSchemes.value
        ? t("admin.acmeApplicationDialog.schemeCount", {
            count: activeCredentialSchemes.value.length,
          })
        : t("admin.acmeApplicationDialog.requiredFieldCount", {
            count: activeCredentialFields.value.length,
          });
    }
    if (hasMultipleCredentialSchemes.value) {
      return t("admin.acmeApplicationDialog.filledAnyScheme", {
        count: filledCredentialCount.value,
      });
    }
    return t("admin.acmeApplicationDialog.filledFieldCount", {
      filled: filledCredentialCount.value,
      total: activeCredentialFields.value.length,
    });
  });

  const providerGroupLabel = (key: string) => {
    if (
      key === "common" ||
      key === "china" ||
      key === "international" ||
      key === "customAdvanced" ||
      key === "other"
    ) {
      return t(`admin.acmeApplicationDialog.providerGroups.${key}`);
    }
    return key;
  };

  const groupedProviders = computed(() => {
    const groupOrder = ["common", "china", "international", "customAdvanced"];
    const bucket = new Map<string, AcmeDnsProvider[]>();
    for (const provider of props.dnsProviders) {
      const group = getProviderGroupKey(provider.group);
      if (!bucket.has(group)) bucket.set(group, []);
      bucket.get(group)!.push(provider);
    }

    const groups = Array.from(bucket.entries()).map(([group, items]) => ({
      group: providerGroupLabel(group),
      groupKey: group,
      items: items
        .slice()
        .sort((a, b) => a.label.localeCompare(b.label, locale.value)),
    }));

    groups.sort((a, b) => {
      const ai = groupOrder.indexOf(a.groupKey);
      const bi = groupOrder.indexOf(b.groupKey);
      if (ai === -1 && bi === -1) {
        return a.group.localeCompare(b.group, locale.value);
      }
      if (ai === -1) return 1;
      if (bi === -1) return -1;
      return ai - bi;
    });
    return groups;
  });

  const dialogTitle = computed(() => {
    return props.mode === "edit"
      ? t("admin.acmeApplicationDialog.editTitle")
      : t("admin.acmeApplicationDialog.createTitle");
  });

  const getCredentialStateKey = (key: string) =>
    `${activeDnsType.value}:${key}`;
  const enableCredentialEditing = (key: string) => {
    credentialEditReady.value[getCredentialStateKey(key)] = true;
  };
  const isCredentialEditReady = (key: string) =>
    credentialEditReady.value[getCredentialStateKey(key)] === true;

  const {
    applySuggestion: applyTransferredCredentials,
    isLoadingSource: isTransferSourceLoading,
    sourceScopeLabel: transferSourceScopeLabel,
    suggestion: credentialTransferSuggestion,
  } = useDnsCredentialTransfer({
    target: "acme",
    providerId: activeDnsType,
    targetCredentials: credentials,
  });

  const credentialTransferDescription = computed(() => {
    const suggestion = credentialTransferSuggestion.value;
    if (!suggestion) return "";
    return t("admin.acmeApplicationDialog.transferDescription", {
      source: transferSourceScopeLabel.value,
      bridge: suggestion.bridgeLabel,
      count: suggestion.fillableFields.length,
    });
  });

  const canSubmit = computed(() => {
    if (!domains.value.length) return false;
    if (!/^dns_[a-z0-9_]+$/i.test(activeDnsType.value)) return false;
    if (!activeCredentialFields.value.length) return true;
    return Boolean(matchedCredentialScheme.value);
  });

  const syncForm = () => {
    const initialValue = props.initialValue;
    name.value = initialValue?.name || "";
    domains.value = Array.isArray(initialValue?.domains)
      ? [...initialValue.domains]
      : [];
    dnsType.value = initialValue?.dnsType || "";
    credentials.value = { ...(initialValue?.credentials || {}) };
    renewEnabled.value = initialValue?.renewEnabled ?? true;
    isCredentialsVisible.value = false;
    credentialEditReady.value = {};
  };

  const handleOpenChange = (nextOpen: boolean) => {
    emit("update:open", nextOpen);
  };

  const submit = (submitNow: boolean) => {
    if (!canSubmit.value) return;
    emit("submit", {
      name: name.value.trim() || undefined,
      domains: domains.value,
      dnsType: activeDnsType.value,
      credentials: buildAcmeCredentialsPayload(credentials.value),
      renewEnabled: renewEnabled.value,
      submitNow,
    });
  };

  const applyCredentialTransfer = () => {
    const result = applyTransferredCredentials();
    if (!result) return;
    for (const key of result.appliedKeys) enableCredentialEditing(key);
    toast.success(
      t("admin.acmeApplicationDialog.transferApplied", {
        source: transferSourceScopeLabel.value,
        count: result.count,
      }),
    );
  };

  watch(
    () => [props.open, props.initialValue] as const,
    ([open]) => {
      if (open) syncForm();
    },
    { immediate: true },
  );

  watch(dnsType, () => {
    credentialEditReady.value = {};
    credentials.value = normalizeProviderCredentials(
      activeProvider.value,
      credentials.value,
    );
    isCredentialsVisible.value = false;
  });

  return {
    activeCredentialFields,
    activeCredentialSchemes,
    activeDnsType,
    applyCredentialTransfer,
    canSubmit,
    credentialSummary,
    credentialTransferDescription,
    credentialTransferSuggestion,
    credentials,
    dialogTitle,
    dnsType,
    domains,
    enableCredentialEditing,
    groupedProviders,
    handleOpenChange,
    hasMultipleCredentialSchemes,
    isCredentialEditReady,
    isCredentialsVisible,
    isTransferSourceLoading,
    name,
    renewEnabled,
    submit,
    t,
    transferSourceScopeLabel,
  };
};
