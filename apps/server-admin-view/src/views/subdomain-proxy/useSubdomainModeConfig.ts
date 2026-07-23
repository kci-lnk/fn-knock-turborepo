import { computed, reactive, watch } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import type { SubdomainModeConfig } from "@/types";
import {
  createDefaultModeForm,
  hasSubdomainRootDomainWildcard,
  normalizePublicPort,
  normalizeRootDomainValue,
  type EdgeClientIpProvider,
  type TranslationParams,
} from "./model";

type ModeConfigSource = {
  subdomain_mode?: SubdomainModeConfig;
};

type SaveSubdomainModeResult = {
  ssl_auto_selection?: {
    applied?: boolean;
    label?: string | null;
    message?: string;
  } | null;
} | void;

export const useSubdomainModeConfig = ({
  getConfig,
  saveSubdomainMode,
  translate,
}: {
  getConfig: () => ModeConfigSource | null | undefined;
  saveSubdomainMode: (
    config: SubdomainModeConfig,
  ) => Promise<SaveSubdomainModeResult>;
  translate: (key: string, params?: TranslationParams) => string;
}) => {
  const modeForm = reactive<SubdomainModeConfig>(createDefaultModeForm());
  const currentModeConfig = computed(
    () => getConfig()?.subdomain_mode ?? createDefaultModeForm(),
  );
  const edgeClientIpProviderOptions = computed<
    Array<{
      value: EdgeClientIpProvider;
      label: string;
      description: string;
      headerHint: string;
    }>
  >(() => [
    {
      value: "tencent_edgeone",
      label: translate(
        "admin.subdomainProxy.edgeProviders.tencentEdgeOne.label",
      ),
      description: translate(
        "admin.subdomainProxy.edgeProviders.tencentEdgeOne.description",
      ),
      headerHint: translate(
        "admin.subdomainProxy.edgeProviders.tencentEdgeOne.headerHint",
      ),
    },
    {
      value: "aliyun_esa",
      label: translate("admin.subdomainProxy.edgeProviders.aliyunEsa.label"),
      description: translate(
        "admin.subdomainProxy.edgeProviders.aliyunEsa.description",
      ),
      headerHint: translate(
        "admin.subdomainProxy.edgeProviders.aliyunEsa.headerHint",
      ),
    },
  ]);

  const getEdgeClientIpProviderLabel = (
    provider: EdgeClientIpProvider | null,
  ): string => {
    if (provider === "tencent_edgeone") {
      return translate(
        "admin.subdomainProxy.edgeProviders.tencentEdgeOne.label",
      );
    }
    if (provider === "aliyun_esa") {
      return translate("admin.subdomainProxy.edgeProviders.aliyunEsa.label");
    }
    return "";
  };

  const savedRootDomain = computed(() =>
    normalizeRootDomainValue(currentModeConfig.value.root_domain),
  );
  const currentDraftRootDomain = computed(() =>
    normalizeRootDomainValue(modeForm.root_domain),
  );
  const isRootDomainPendingSave = computed(
    () => currentDraftRootDomain.value !== savedRootDomain.value,
  );
  const rootDomainValidationMessage = computed(() =>
    hasSubdomainRootDomainWildcard(modeForm.root_domain)
      ? translate("admin.subdomainProxy.rootDomainWildcardForbidden")
      : "",
  );
  const isModeValid = computed(() => !rootDomainValidationMessage.value);
  const canUseRootDomainSuffix = computed(
    () =>
      Boolean(savedRootDomain.value) &&
      !isRootDomainPendingSave.value &&
      isModeValid.value,
  );
  const canManageNewMappings = computed(
    () =>
      Boolean(savedRootDomain.value) &&
      !isRootDomainPendingSave.value &&
      isModeValid.value,
  );
  const isModeDirty = computed(
    () => JSON.stringify(modeForm) !== JSON.stringify(currentModeConfig.value),
  );

  const { isPending: isSavingMode, run: runSaveMode } = useAsyncAction({
    onError: (error) => {
      toast.error(translate("admin.subdomainProxy.saveFailed"), {
        description: extractErrorMessage(
          error,
          translate("admin.subdomainProxy.saveModeFailed"),
        ),
      });
    },
  });

  const applyModeForm = (next: SubdomainModeConfig) => {
    modeForm.root_domain = next.root_domain;
    modeForm.auth_host = next.auth_host;
    modeForm.auth_target = next.auth_target;
    modeForm.cookie_domain = next.cookie_domain;
    modeForm.edge_client_ip_enabled = next.edge_client_ip_enabled;
    modeForm.aliyun_esa_enabled = next.aliyun_esa_enabled;
    modeForm.tencent_edgeone_enabled = next.tencent_edgeone_enabled;
    modeForm.public_auth_base_url = next.public_auth_base_url;
    modeForm.public_http_port = normalizePublicPort(next.public_http_port);
    modeForm.public_https_port = normalizePublicPort(next.public_https_port);
    modeForm.auth_cache_ttl_seconds = next.auth_cache_ttl_seconds;
    modeForm.auth_cache_unauthorized_ttl_seconds =
      next.auth_cache_unauthorized_ttl_seconds;
    modeForm.default_access_mode = next.default_access_mode;
    modeForm.auto_add_whitelist_on_login = next.auto_add_whitelist_on_login;
    modeForm.passkey_rp_mode = next.passkey_rp_mode;
    modeForm.passkey_rp_id = next.passkey_rp_id || "";
  };

  const resetModeForm = () => {
    applyModeForm(currentModeConfig.value);
  };

  const saveMode = async () => {
    if (!isModeValid.value || !isModeDirty.value) return;
    await runSaveMode(async () => {
      const result = await saveSubdomainMode({
        ...modeForm,
        root_domain: modeForm.root_domain.trim().toLowerCase(),
        auth_host: modeForm.auth_host.trim().toLowerCase(),
        auth_target: modeForm.auth_target.trim(),
        cookie_domain: modeForm.cookie_domain.trim(),
        edge_client_ip_enabled: modeForm.edge_client_ip_enabled,
        aliyun_esa_enabled: modeForm.aliyun_esa_enabled,
        tencent_edgeone_enabled: modeForm.tencent_edgeone_enabled,
        public_auth_base_url: modeForm.public_auth_base_url.trim(),
        public_http_port: normalizePublicPort(modeForm.public_http_port),
        public_https_port: normalizePublicPort(modeForm.public_https_port),
        auth_cache_ttl_seconds: Math.max(
          0,
          Math.floor(Number(modeForm.auth_cache_ttl_seconds) || 0),
        ),
        auth_cache_unauthorized_ttl_seconds: Math.max(
          0,
          Math.floor(Number(modeForm.auth_cache_unauthorized_ttl_seconds) || 0),
        ),
        passkey_rp_id: (modeForm.passkey_rp_id || "").trim().toLowerCase(),
      });
      toast.success(translate("admin.subdomainProxy.modeSaved"));
      if (result?.ssl_auto_selection?.message) {
        if (result.ssl_auto_selection.applied) {
          toast.success(result.ssl_auto_selection.message, {
            description: result.ssl_auto_selection.label
              ? translate("admin.subdomainProxy.switchedCertificate", {
                  label: result.ssl_auto_selection.label,
                })
              : undefined,
          });
        } else {
          toast.error(
            translate("admin.subdomainProxy.sslAutoSwitchIncomplete"),
            {
              description: result.ssl_auto_selection.message,
            },
          );
        }
      }
    });
  };

  watch(
    () => getConfig()?.subdomain_mode,
    (next) => {
      if (next) {
        applyModeForm(next);
      }
    },
    { immediate: true },
  );

  watch(
    () =>
      [
        modeForm.edge_client_ip_enabled,
        modeForm.aliyun_esa_enabled,
        modeForm.tencent_edgeone_enabled,
      ] as const,
    ([enabled, aliyunEnabled, tencentEnabled]) => {
      if (!enabled) {
        if (modeForm.aliyun_esa_enabled) {
          modeForm.aliyun_esa_enabled = false;
        }
        if (modeForm.tencent_edgeone_enabled) {
          modeForm.tencent_edgeone_enabled = false;
        }
        return;
      }

      if (tencentEnabled && aliyunEnabled) {
        modeForm.aliyun_esa_enabled = false;
        return;
      }

      if (!aliyunEnabled && !tencentEnabled) {
        modeForm.aliyun_esa_enabled = true;
      }
    },
  );

  return {
    canManageNewMappings,
    canUseRootDomainSuffix,
    currentDraftRootDomain,
    currentModeConfig,
    edgeClientIpProviderOptions,
    getEdgeClientIpProviderLabel,
    isModeDirty,
    isModeValid,
    isRootDomainPendingSave,
    isSavingMode,
    modeForm,
    resetModeForm,
    rootDomainValidationMessage,
    saveMode,
    savedRootDomain,
  };
};
