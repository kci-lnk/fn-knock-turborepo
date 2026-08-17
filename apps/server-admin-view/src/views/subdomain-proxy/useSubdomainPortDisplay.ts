import { computed, type ComputedRef, type Ref } from "vue";
import type { AppConfig, SubdomainModeConfig } from "@/types";
import {
  isCloudflaredReverseProxySubdomainMode,
  isReverseProxySubdomainMode,
} from "@/lib/reverse-proxy-submode";
import {
  formatHostWithOptionalPort,
  isDefaultPublicPort,
  normalizePublicPort,
  resolveConfiguredAccessEntryPublicPort,
  resolveConfiguredAuthServicePublicPort,
  resolveEdgeClientIpProvider,
  syncPublicAuthBaseUrlPort,
  type EdgeClientIpProvider,
} from "./model";

export const useSubdomainPortDisplay = ({
  accessEntryPort,
  currentModeConfig,
  getConfig,
  modeForm,
}: {
  accessEntryPort: Ref<string>;
  currentModeConfig: ComputedRef<SubdomainModeConfig>;
  getConfig: () => AppConfig | null;
  modeForm: SubdomainModeConfig;
}) => {
  const defaultAuthServicePublicPort = computed(
    () => normalizePublicPort(accessEntryPort.value) || 7999,
  );
  const isReverseProxySubdomain = computed(() =>
    isReverseProxySubdomainMode(getConfig()),
  );
  const isCloudflaredReverseProxySubdomain = computed(() =>
    isCloudflaredReverseProxySubdomainMode(getConfig()),
  );
  const isFrpReverseProxySubdomain = computed(
    () =>
      isReverseProxySubdomain.value &&
      !isCloudflaredReverseProxySubdomain.value,
  );
  const configuredAuthServicePublicPort = computed(() =>
    resolveConfiguredAuthServicePublicPort(
      modeForm,
      !isFrpReverseProxySubdomain.value,
    ),
  );
  const authServicePublicPort = computed({
    get: () => {
      return (
        configuredAuthServicePublicPort.value ||
        defaultAuthServicePublicPort.value
      );
    },
    set: (value: number | string) => {
      const port = normalizePublicPort(value);
      modeForm.public_https_port = port || 0;
      modeForm.public_http_port = 0;
      modeForm.public_auth_base_url = syncPublicAuthBaseUrlPort(
        modeForm.public_auth_base_url,
        port,
      );
    },
  });
  const draftAuthServicePublicPort = computed(() =>
    String(authServicePublicPort.value || defaultAuthServicePublicPort.value),
  );
  const configuredAccessEntryPort = computed(() =>
    resolveConfiguredAccessEntryPublicPort(
      currentModeConfig.value,
      !isFrpReverseProxySubdomain.value,
    ),
  );
  const displayAccessEntryPort = computed(() =>
    configuredAccessEntryPort.value > 0
      ? String(configuredAccessEntryPort.value)
      : accessEntryPort.value.trim() || "7999",
  );
  const isEdgeClientIPModeEditable = computed(
    () => getConfig()?.run_type === 3,
  );
  const resolvedSavedEdgeClientIpProvider = computed(() =>
    resolveEdgeClientIpProvider(currentModeConfig.value),
  );
  const savedEdgeClientIpProvider = computed(() =>
    isEdgeClientIPModeEditable.value
      ? resolvedSavedEdgeClientIpProvider.value
      : null,
  );
  const isSavedEdgeClientIPActive = computed(
    () => savedEdgeClientIpProvider.value !== null,
  );
  const activeEdgeClientIpProvider = computed(() =>
    isEdgeClientIPModeEditable.value
      ? resolveEdgeClientIpProvider(modeForm)
      : null,
  );
  const isEdgeClientIPActive = computed(
    () =>
      isEdgeClientIPModeEditable.value &&
      activeEdgeClientIpProvider.value !== null,
  );
  const omitPublicPortConfiguration = computed(
    () => isReverseProxySubdomain.value,
  );
  const shouldOmitAccessEntryPort = computed(() => {
    if (
      isSavedEdgeClientIPActive.value ||
      isCloudflaredReverseProxySubdomain.value
    ) {
      return true;
    }
    return isDefaultPublicPort(displayAccessEntryPort.value);
  });
  const formatHostWithAccessEntryPort = (host: string): string =>
    formatHostWithOptionalPort(
      host,
      displayAccessEntryPort.value,
      shouldOmitAccessEntryPort.value,
    );
  const shouldOmitDraftAuthServicePublicPort = computed(() => {
    if (
      isEdgeClientIPActive.value ||
      isCloudflaredReverseProxySubdomain.value
    ) {
      return true;
    }
    return isDefaultPublicPort(authServicePublicPort.value);
  });
  const formatAuthServiceHostWithPublicPort = (host: string): string =>
    formatHostWithOptionalPort(
      host,
      draftAuthServicePublicPort.value,
      shouldOmitDraftAuthServicePublicPort.value,
    );

  const selectEdgeClientIpProvider = (provider: EdgeClientIpProvider) => {
    if (!isEdgeClientIPModeEditable.value) return;

    modeForm.edge_client_ip_enabled = true;
    modeForm.aliyun_esa_enabled = provider === "aliyun_esa";
    modeForm.tencent_edgeone_enabled = provider === "tencent_edgeone";
  };

  return {
    activeEdgeClientIpProvider,
    authServicePublicPort,
    formatAuthServiceHostWithPublicPort,
    formatHostWithAccessEntryPort,
    isEdgeClientIPModeEditable,
    omitPublicPortConfiguration,
    savedEdgeClientIpProvider,
    selectEdgeClientIpProvider,
  };
};
