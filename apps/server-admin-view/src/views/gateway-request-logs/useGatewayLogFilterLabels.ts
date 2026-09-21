import { computed, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import type { TOTPCredential } from "@/types";
import {
  LOGIN_FILTER_OPTIONS,
  STATUS_FILTER_OPTIONS,
  WAF_FILTER_OPTIONS,
  UNRECORDED_CREDENTIAL_FILTER,
  getGatewayLogOptionLabel,
} from "./model";

export const useGatewayLogFilterLabels = (filters: {
  selectedStatus: Ref<string>;
  selectedLoggedIn: Ref<string>;
  selectedCredential: Ref<string>;
  selectedWAFStatus: Ref<string>;
  credentialOptions: Ref<TOTPCredential[]>;
}) => {
  const {
    selectedStatus,
    selectedLoggedIn,
    selectedCredential,
    selectedWAFStatus,
    credentialOptions,
  } = filters;
  const { t } = useI18n();
  const activeStatusLabel = computed(() =>
    getGatewayLogOptionLabel(
      STATUS_FILTER_OPTIONS,
      selectedStatus.value,
      "admin.gatewayRequestLogs.statusFilters.all",
      t,
    ),
  );
  const activeLoggedInLabel = computed(() =>
    getGatewayLogOptionLabel(
      LOGIN_FILTER_OPTIONS,
      selectedLoggedIn.value,
      "admin.gatewayRequestLogs.loginFilters.all",
      t,
    ),
  );
  const credentialFilterOptions = computed(() => {
    const options = [
      {
        value: "all",
        label: t("admin.gatewayRequestLogs.credentialFilters.all"),
      },
      {
        value: UNRECORDED_CREDENTIAL_FILTER,
        label: t("admin.gatewayRequestLogs.credentialFilters.unrecorded"),
      },
      ...credentialOptions.value.map((credential) => ({
        value: credential.id,
        label: credential.comment?.trim() || credential.id,
      })),
    ];
    if (
      selectedCredential.value !== "all" &&
      !options.some((option) => option.value === selectedCredential.value)
    ) {
      options.push({
        value: selectedCredential.value,
        label: selectedCredential.value,
      });
    }
    return options;
  });
  const activeCredentialLabel = computed(
    () =>
      credentialFilterOptions.value.find(
        (option) => option.value === selectedCredential.value,
      )?.label || selectedCredential.value,
  );
  const activeWAFStatusLabel = computed(() =>
    getGatewayLogOptionLabel(
      WAF_FILTER_OPTIONS,
      selectedWAFStatus.value,
      "admin.gatewayRequestLogs.wafFilters.all",
      t,
    ),
  );
  return {
    activeStatusLabel,
    activeLoggedInLabel,
    credentialFilterOptions,
    activeCredentialLabel,
    activeWAFStatusLabel,
  };
};
