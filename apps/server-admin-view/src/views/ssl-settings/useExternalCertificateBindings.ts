import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import type {
  ExternalCertificateBinding,
  ExternalCertificateBindingCredential,
} from "@/types";

export type ExternalCertificateCredentialField = {
  label: string;
  value: string;
  multiline?: boolean;
};

const PROVIDER_NAMES = {
  certd: "Certd",
  acme_sh: "acme.sh",
  lego: "lego",
  certbot: "Certbot",
} as const satisfies Record<ExternalCertificateBinding["provider"], string>;

export function useExternalCertificateBindings() {
  const { t, locale } = useI18n();
  const bindings = ref<ExternalCertificateBinding[]>([]);
  const bindingNameDrafts = ref<Record<string, string>>({});
  const credential = ref<ExternalCertificateBindingCredential | null>(null);
  const provider = ref<ExternalCertificateBinding["provider"]>("certd");
  const bindingName = ref("Certd");
  const isLoading = ref(true);
  const isCreating = ref(false);
  const pendingBindingId = ref<string | null>(null);

  const configured = computed(() => bindings.value.length > 0);
  const providerOptions = Object.entries(PROVIDER_NAMES).map(
    ([value, label]) => ({
      value: value as ExternalCertificateBinding["provider"],
      label,
    }),
  );
  const summary = computed(() =>
    bindings.value.length
      ? t("admin.certConfig.externalBindingsSummary", {
          count: bindings.value.length,
        })
      : t("admin.certConfig.externalNoBindingsSummary"),
  );
  const credentialFields = computed<ExternalCertificateCredentialField[]>(
    () => {
      const value = credential.value;
      if (!value) return [];
      const publicUrl = value.binding.public_deploy_url ?? "";
      const localUrl = localDeployUrl(value.binding);
      if (value.binding.setup_kind === "deploy_hook") {
        const script = renderDeployHook(
          value.binding,
          preferredDeployUrl(value.binding),
          value.token,
        );
        return [
          {
            label: t("admin.certConfig.externalUsageLabel"),
            value: value.binding.usage_instructions ?? "",
            multiline: true,
          },
          {
            label: t("admin.certConfig.externalScriptLabel"),
            value: script,
            multiline: true,
          },
          {
            label: t("admin.certConfig.externalLocalUrlLabel"),
            value: localUrl,
          },
        ].filter((field) => field.value);
      }
      return [
        {
          label: t("admin.certConfig.externalMethodLabel"),
          value: value.binding.request_method ?? "",
        },
        {
          label: t("admin.certConfig.externalPublicUrlLabel"),
          value: publicUrl,
        },
        {
          label: t("admin.certConfig.externalLocalUrlLabel"),
          value: localUrl,
        },
        {
          label: t("admin.certConfig.externalHeaderLabel"),
          value: authorizationHeader(value.token),
        },
        {
          label: t("admin.certConfig.externalBodyLabel"),
          value: value.binding.request_body_template ?? "",
        },
        {
          label: t("admin.certConfig.externalSuccessMarkerLabel"),
          value: value.binding.success_marker ?? "",
        },
      ].filter((field) => field.value);
    },
  );

  watch(provider, (nextProvider, previousProvider) => {
    const currentName = bindingName.value.trim();
    if (!currentName || currentName === PROVIDER_NAMES[previousProvider]) {
      bindingName.value = PROVIDER_NAMES[nextProvider];
    }
  });

  async function loadBindings() {
    isLoading.value = true;
    try {
      bindings.value = await ConfigAPI.getExternalCertificateBindings();
      bindingNameDrafts.value = Object.fromEntries(
        bindings.value.map((binding) => [binding.id, binding.name]),
      );
    } catch (error) {
      showError(error, "admin.certConfig.externalLoadFailed");
    } finally {
      isLoading.value = false;
    }
  }

  function replaceBinding(binding: ExternalCertificateBinding) {
    const index = bindings.value.findIndex((item) => item.id === binding.id);
    if (index === -1) bindings.value.push(binding);
    else bindings.value.splice(index, 1, binding);
    bindingNameDrafts.value[binding.id] = binding.name;
  }

  async function createBinding() {
    const name = bindingName.value.trim();
    if (!name) return;
    isCreating.value = true;
    try {
      const result = await ConfigAPI.createExternalCertificateBinding(
        name,
        provider.value,
      );
      credential.value = result;
      replaceBinding(result.binding);
      toast.success(t("admin.certConfig.externalCreateSuccess"));
    } catch (error) {
      showError(error, "admin.certConfig.externalCreateFailed");
    } finally {
      isCreating.value = false;
    }
  }

  async function setBindingEnabled(
    binding: ExternalCertificateBinding,
    enabled: boolean,
  ) {
    pendingBindingId.value = binding.id;
    try {
      replaceBinding(
        await ConfigAPI.updateExternalCertificateBinding(binding.id, {
          enabled,
        }),
      );
      toast.success(
        enabled
          ? t("admin.certConfig.externalEnabledSuccess")
          : t("admin.certConfig.externalDisabledSuccess"),
      );
    } catch (error) {
      showError(error, "admin.certConfig.externalUpdateFailed");
    } finally {
      pendingBindingId.value = null;
    }
  }

  async function renameBinding(binding: ExternalCertificateBinding) {
    const name = (bindingNameDrafts.value[binding.id] ?? "").trim();
    if (!name || name === binding.name) return;
    pendingBindingId.value = binding.id;
    try {
      replaceBinding(
        await ConfigAPI.updateExternalCertificateBinding(binding.id, { name }),
      );
      toast.success(t("admin.certConfig.externalRenameSuccess"));
    } catch (error) {
      showError(error, "admin.certConfig.externalUpdateFailed");
    } finally {
      pendingBindingId.value = null;
    }
  }

  async function rotateToken(binding: ExternalCertificateBinding) {
    pendingBindingId.value = binding.id;
    try {
      const result = await ConfigAPI.rotateExternalCertificateBindingToken(
        binding.id,
      );
      credential.value = result;
      replaceBinding(result.binding);
      toast.success(t("admin.certConfig.externalRotateSuccess"));
    } catch (error) {
      showError(error, "admin.certConfig.externalRotateFailed");
    } finally {
      pendingBindingId.value = null;
    }
  }

  async function revokeBinding(binding: ExternalCertificateBinding) {
    pendingBindingId.value = binding.id;
    try {
      await ConfigAPI.deleteExternalCertificateBinding(binding.id);
      bindings.value = bindings.value.filter((item) => item.id !== binding.id);
      delete bindingNameDrafts.value[binding.id];
      if (credential.value?.binding.id === binding.id) credential.value = null;
      toast.success(t("admin.certConfig.externalRevokeSuccess"));
    } catch (error) {
      showError(error, "admin.certConfig.externalRevokeFailed");
    } finally {
      pendingBindingId.value = null;
    }
  }

  async function copyValue(value: string) {
    try {
      await copyTextToClipboard(value);
      toast.success(t("admin.certConfig.externalCopied"));
    } catch (error) {
      showError(error, "admin.certConfig.externalCopyFailed");
    }
  }

  async function copyCompleteConfiguration() {
    if (!credential.value) return;
    if (credential.value.binding.setup_kind === "deploy_hook") {
      const script = credentialFields.value.find(
        (field) => field.label === t("admin.certConfig.externalScriptLabel"),
      );
      if (script) {
        await copyValue(script.value);
        return;
      }
    }
    await copyValue(
      credentialFields.value
        .map((field) => `${field.label}: ${field.value}`)
        .join("\n"),
    );
  }

  function clearCredential() {
    credential.value = null;
  }

  function formatDate(value?: string | null) {
    if (!value) return t("admin.certConfig.externalNeverDeployed");
    const date = new Date(value);
    if (Number.isNaN(date.getTime())) return value;
    return new Intl.DateTimeFormat(locale.value, {
      dateStyle: "medium",
      timeStyle: "short",
    }).format(date);
  }

  function showError(error: unknown, fallbackKey: string) {
    toast.error(extractErrorMessage(error, t(fallbackKey)));
  }

  return {
    bindingName,
    bindingNameDrafts,
    bindings,
    configured,
    clearCredential,
    copyCompleteConfiguration,
    copyValue,
    createBinding,
    credential,
    credentialFields,
    formatDate,
    isCreating,
    isLoading,
    loadBindings,
    pendingBindingId,
    provider,
    providerOptions,
    publicDeployStatusDescription,
    providerName,
    renameBinding,
    revokeBinding,
    rotateToken,
    setBindingEnabled,
    summary,
  };
}

function localDeployUrl(binding: ExternalCertificateBinding) {
  return new URL(
    binding.deploy_path,
    `http://127.0.0.1:${binding.deploy_port}`,
  ).toString();
}

function preferredDeployUrl(binding: ExternalCertificateBinding) {
  return binding.public_deploy_status === "ready" && binding.public_deploy_url
    ? binding.public_deploy_url
    : localDeployUrl(binding);
}

function publicDeployStatusDescription(binding: ExternalCertificateBinding) {
  switch (binding.public_deploy_status) {
    case "ready":
      return "admin.certConfig.externalPublicReadyDescription";
    case "https_required":
      return "admin.certConfig.externalPublicHttpsRequiredDescription";
    default:
      return "admin.certConfig.externalPublicUnconfiguredDescription";
  }
}

function authorizationHeader(token: string) {
  return `Authorization=Bearer ${token}`;
}

function providerName(provider: ExternalCertificateBinding["provider"]) {
  return PROVIDER_NAMES[provider];
}

function renderDeployHook(
  binding: ExternalCertificateBinding,
  deployUrl: string,
  token: string,
) {
  return (binding.script_template ?? "")
    .split("__FN_KNOCK_DEPLOY_URL__")
    .join(deployUrl)
    .split("__FN_KNOCK_DEPLOY_TOKEN__")
    .join(token);
}
