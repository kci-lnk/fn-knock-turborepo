import { computed, onMounted, reactive, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import type {
  ExternalAuthProviderType,
  OIDCProviderCatalogItem,
  OIDCProviderView,
} from "@/types";
import {
  hasOidcConnectionValue,
  normalizeOidcScopes,
  oidcConnectionValueText,
  type OIDCProviderForm,
} from "./oidcProviderForm";

export const useOIDCProviderManagement = () => {
  const { t } = useI18n();
  const router = useRouter();
  const catalog = ref<OIDCProviderCatalogItem[]>([]);
  const providers = ref<OIDCProviderView[]>([]);
  const form = reactive<OIDCProviderForm>({
    type: "google",
    name: "",
    clientId: "",
    clientSecret: "",
    issuer: "",
    tenant: "common",
    scopes: "",
  });
  const editForm = reactive<OIDCProviderForm>({
    id: "",
    type: "google",
    name: "",
    enabled: false,
    clientId: "",
    clientSecret: "",
    issuer: "",
    tenant: "common",
    scopes: "",
  });
  const showCreateDialog = ref(false);
  const showQqBindingAlert = ref(false);
  const showEditDialog = ref(false);
  const selectedDefinition = computed(() =>
    catalog.value.find((item) => item.type === form.type),
  );

  const { isPending: isLoading, run: runLoad } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.oidcProviders.loadFailed")),
      );
    },
  });
  const { isPending: isSaving, run: runSave } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.oidcProviders.saveFailed")),
      );
    },
  });
  const { isPending: isMutating, run: runMutate } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(error, t("admin.oidcProviders.operationFailed")),
      );
    },
  });

  const resetCreateForm = () => {
    const definition =
      catalog.value.find((item) => item.type === form.type) || catalog.value[0];
    form.type = (definition?.type || "google") as ExternalAuthProviderType;
    form.name = definition?.default_name || "";
    form.clientId = "";
    form.clientSecret = "";
    form.issuer = "";
    form.tenant = "common";
    form.scopes = definition?.default_scopes.join(" ") || "";
  };

  const loadAll = async () => {
    await runLoad(async () => {
      const [catalogData, providersData] = await Promise.all([
        ConfigAPI.getOIDCProviderCatalog(),
        ConfigAPI.getOIDCProviders(),
      ]);
      catalog.value = catalogData;
      providers.value = providersData;
      if (!catalog.value.some((item) => item.type === form.type)) {
        resetCreateForm();
      }
    });
  };

  const openCreateDialog = () => {
    resetCreateForm();
    showCreateDialog.value = true;
  };

  const handleCreateProviderTypeChange = (value: unknown) => {
    form.type = String(value ?? "") as ExternalAuthProviderType;
    const definition = catalog.value.find((item) => item.type === form.type);
    form.name = definition?.default_name || "";
    form.scopes = definition?.default_scopes.join(" ") || "";
    form.issuer = "";
    form.tenant = "common";
  };

  const providerLabel = (type: string) =>
    catalog.value.find((item) => item.type === type)?.label || type;

  const isCreateConfigComplete = () => {
    const definition = selectedDefinition.value;
    if (!definition) return false;
    if (definition.type === "fnknock_qq") return true;
    const values: Record<string, unknown> = {
      client_id: form.clientId.trim(),
      client_secret: form.clientSecret.trim(),
      issuer:
        form.type === "custom_oidc"
          ? form.issuer.trim()
          : form.type === "microsoft" && form.tenant.trim()
            ? `https://login.microsoftonline.com/${form.tenant.trim()}/v2.0`
            : undefined,
    };
    return definition.required_fields.every((field) =>
      hasOidcConnectionValue(values[field]),
    );
  };

  const providerHasRequiredConfig = (provider: OIDCProviderView) => {
    const definition = catalog.value.find(
      (item) => item.type === provider.type,
    );
    return Boolean(
      definition?.required_fields.every((field) =>
        hasOidcConnectionValue(provider.connection_config_masked[field]),
      ),
    );
  };

  const providerStatus = (provider: OIDCProviderView) => {
    if (!providerHasRequiredConfig(provider)) {
      return t("admin.oidcProviders.pendingConfig");
    }
    return provider.enabled
      ? t("admin.oidcProviders.enabled")
      : t("admin.oidcProviders.disabled");
  };

  const copyCallbackUrl = async (url: string) => {
    try {
      await copyTextToClipboard(url);
      toast.success(t("admin.oidcProviders.callbackCopied"), {
        description: url,
      });
    } catch (error) {
      console.error("copyCallbackUrl:", error);
      toast.error(t("admin.oidcProviders.callbackCopyFailed"), {
        description: t("admin.oidcProviders.copyRestricted"),
      });
    }
  };

  const handleCreateProvider = async () => {
    await runSave(async () => {
      const isQqProvider = form.type === "fnknock_qq";
      const scopes = normalizeOidcScopes(form.scopes);
      const enabled = isCreateConfigComplete();
      const provider = await ConfigAPI.createOIDCProvider({
        type: form.type,
        name: form.name.trim(),
        enabled: isQqProvider ? false : enabled,
        connection_config: {
          client_id: form.clientId.trim(),
          client_secret: form.clientSecret.trim(),
          ...(form.type === "custom_oidc"
            ? { issuer: form.issuer.trim() }
            : {}),
          ...(form.type === "microsoft" ? { tenant: form.tenant.trim() } : {}),
          ...(scopes.length ? { scopes } : {}),
        },
      });
      if (isQqProvider) {
        const testResult = await ConfigAPI.testOIDCProvider(provider.id);
        if (!testResult.success) {
          try {
            await ConfigAPI.deleteOIDCProvider(provider.id);
          } catch {
            // The provider remains disabled if cleanup fails.
          }
          throw new Error(
            testResult.message || t("admin.oidcProviders.operationFailed"),
          );
        }
        await ConfigAPI.updateOIDCProvider(provider.id, { enabled: true });
      }
      form.clientId = "";
      form.clientSecret = "";
      showCreateDialog.value = false;
      toast.success(
        enabled
          ? t("admin.oidcProviders.providerAdded")
          : t("admin.oidcProviders.providerDraftAdded"),
      );
      await loadAll();
      if (isQqProvider) showQqBindingAlert.value = true;
    });
  };

  const returnToTotpManagement = async () => {
    showQqBindingAlert.value = false;
    await router.push({ name: "AuthSettings" });
  };

  const openEditDialog = (provider: OIDCProviderView) => {
    const config = provider.connection_config_masked || {};
    editForm.id = provider.id;
    editForm.type = provider.type;
    editForm.name = provider.name;
    editForm.enabled = provider.enabled;
    editForm.clientId = oidcConnectionValueText(config.client_id);
    editForm.clientSecret = "";
    editForm.issuer = oidcConnectionValueText(config.issuer);
    editForm.tenant = oidcConnectionValueText(config.tenant) || "common";
    editForm.scopes = oidcConnectionValueText(config.scopes);
    showEditDialog.value = true;
  };

  const saveProviderEdit = async () => {
    if (!editForm.id) return;
    await runMutate(async () => {
      const scopes = normalizeOidcScopes(editForm.scopes);
      await ConfigAPI.updateOIDCProvider(editForm.id!, {
        name: editForm.name.trim(),
        enabled: editForm.enabled === true,
        connection_config: {
          client_id: editForm.clientId.trim(),
          ...(editForm.clientSecret.trim()
            ? { client_secret: editForm.clientSecret.trim() }
            : {}),
          ...(editForm.type === "custom_oidc"
            ? { issuer: editForm.issuer.trim() }
            : {}),
          ...(editForm.type === "microsoft"
            ? { tenant: editForm.tenant.trim() }
            : {}),
          ...(scopes.length ? { scopes } : {}),
        },
      });
      toast.success(t("admin.oidcProviders.providerSaved"));
      showEditDialog.value = false;
      await loadAll();
    });
  };

  const deleteProvider = async (id: string) => {
    await runMutate(async () => {
      await ConfigAPI.deleteOIDCProvider(id);
      toast.success(t("admin.oidcProviders.providerDeleted"));
      await loadAll();
    });
  };

  watch(
    selectedDefinition,
    (definition) => {
      if (!definition) return;
      if (!form.name.trim()) form.name = definition.default_name;
      form.scopes = definition.default_scopes.join(" ");
      if (definition.type === "microsoft" && !form.tenant.trim()) {
        form.tenant = "common";
      }
    },
    { immediate: true },
  );
  onMounted(loadAll);

  return {
    catalog,
    copyCallbackUrl,
    deleteProvider,
    editForm,
    form,
    handleCreateProvider,
    handleCreateProviderTypeChange,
    isLoading,
    isMutating,
    isSaving,
    openCreateDialog,
    openEditDialog,
    providerLabel,
    providers,
    providerStatus,
    returnToTotpManagement,
    saveProviderEdit,
    showCreateDialog,
    showEditDialog,
    showQqBindingAlert,
  };
};
