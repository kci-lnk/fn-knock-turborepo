import { onBeforeUnmount, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { copyTextToClipboard } from "@admin-shared/utils/copyTextToClipboard";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api";
import type {
  LdapBinding,
  LdapProviderView,
  OIDCBinding,
  OIDCProviderView,
} from "@/types";

export type ExternalProviderOption =
  (OIDCProviderView & { kind: "oidc" }) | (LdapProviderView & { kind: "ldap" });
export type ExternalBinding =
  (OIDCBinding & { protocol: "oidc" }) | (LdapBinding & { protocol: "ldap" });

const AUTO_REFRESH_INTERVAL_MS = 5_000;

export const useOidcBindingWorkflow = (options: {
  setError: (message: string) => void;
  totpId: string;
}) => {
  const { t } = useI18n();
  const oidcBindings = ref<ExternalBinding[]>([]);
  const providers = ref<ExternalProviderOption[]>([]);
  const showInviteDialog = ref(false);
  const inviteProviderId = ref("");
  const inviteUrl = ref("");
  const inviteExpiresAt = ref("");
  const isOidcBindingsRefreshing = ref(false);
  let autoRefreshTimer: ReturnType<typeof window.setInterval> | null = null;

  const { isPending: isInviteCreating, run: runCreateInvite } = useAsyncAction({
    onError: (error) => {
      toast.error(
        extractErrorMessage(
          error,
          t("admin.passkeySettings.createInviteFailed"),
        ),
      );
    },
  });
  const { isPending: isDeletingBinding, run: runDeleteBinding } =
    useAsyncAction({
      onError: (error) => {
        const message = extractErrorMessage(
          error,
          t("admin.passkeySettings.deleteFailed"),
        );
        options.setError(message);
        toast.error(t("admin.passkeySettings.deleteErrorTitle"), {
          description: message,
        });
      },
    });

  const loadOidcData = async () => {
    const [
      oidcBindingList,
      oidcProviderList,
      ldapBindingList,
      ldapProviderList,
    ] = await Promise.all([
      ConfigAPI.getOIDCBindings(options.totpId),
      ConfigAPI.getOIDCProviders(),
      ConfigAPI.getLdapBindings(options.totpId),
      ConfigAPI.getLdapProviders(),
    ]);
    oidcBindings.value = [
      ...oidcBindingList.map((binding) => ({
        ...binding,
        protocol: "oidc" as const,
      })),
      ...ldapBindingList.map((binding) => ({
        ...binding,
        protocol: "ldap" as const,
      })),
    ];
    providers.value = [
      ...oidcProviderList
        .filter((provider) => provider.enabled)
        .map((provider) => ({ ...provider, kind: "oidc" as const })),
      ...ldapProviderList
        .filter((provider) => provider.enabled)
        .map((provider) => ({ ...provider, kind: "ldap" as const })),
    ];
  };

  const formatOidcBindingLabel = (binding: ExternalBinding) =>
    binding.display_name ||
    binding.email ||
    (binding.protocol === "ldap" ? binding.username : undefined) ||
    binding.provider_name ||
    binding.provider_type;

  const refreshOidcBindings = async (refreshOptions?: {
    notifyOnAdded?: boolean;
    showSuccessToast?: boolean;
    showErrorToast?: boolean;
  }) => {
    if (isOidcBindingsRefreshing.value) return;
    const previousIds = new Set(
      oidcBindings.value.map((binding) => binding.id),
    );
    isOidcBindingsRefreshing.value = true;
    try {
      const [nextOidcBindings, nextLdapBindings] = await Promise.all([
        ConfigAPI.getOIDCBindings(options.totpId),
        ConfigAPI.getLdapBindings(options.totpId),
      ]);
      const nextBindings: ExternalBinding[] = [
        ...nextOidcBindings.map((binding) => ({
          ...binding,
          protocol: "oidc" as const,
        })),
        ...nextLdapBindings.map((binding) => ({
          ...binding,
          protocol: "ldap" as const,
        })),
      ];
      const addedBindings = nextBindings.filter(
        (binding) => !previousIds.has(binding.id),
      );
      oidcBindings.value = nextBindings;
      options.setError("");
      if (refreshOptions?.notifyOnAdded && addedBindings.length > 0) {
        const firstBinding = addedBindings[0];
        if (!firstBinding) return;
        toast.success(
          addedBindings.length > 1
            ? t("admin.passkeySettings.addedBindingsMany", {
                count: addedBindings.length,
              })
            : t("admin.passkeySettings.addedBindingOne"),
          { description: formatOidcBindingLabel(firstBinding) },
        );
      } else if (refreshOptions?.showSuccessToast) {
        toast.success(t("admin.passkeySettings.bindingsRefreshed"));
      }
    } catch (error) {
      const message = extractErrorMessage(
        error,
        t("admin.passkeySettings.refreshFailed"),
      );
      options.setError(message);
      if (refreshOptions?.showErrorToast) {
        toast.error(t("admin.passkeySettings.refreshErrorTitle"), {
          description: message,
        });
      } else {
        console.error("refreshOidcBindings:", error);
      }
    } finally {
      isOidcBindingsRefreshing.value = false;
    }
  };

  const handleRefreshOidcBindings = () => {
    void refreshOidcBindings({
      notifyOnAdded: showInviteDialog.value,
      showSuccessToast: !showInviteDialog.value,
      showErrorToast: true,
    });
  };

  const stopAutoRefresh = () => {
    if (autoRefreshTimer === null) return;
    window.clearInterval(autoRefreshTimer);
    autoRefreshTimer = null;
  };

  const startAutoRefresh = () => {
    stopAutoRefresh();
    autoRefreshTimer = window.setInterval(() => {
      void refreshOidcBindings({ notifyOnAdded: true });
    }, AUTO_REFRESH_INTERVAL_MS);
  };

  const openInviteDialog = () => {
    inviteProviderId.value = providers.value[0]?.id || "";
    inviteUrl.value = "";
    inviteExpiresAt.value = "";
    showInviteDialog.value = true;
  };

  const handleInviteProviderChange = (value: unknown) => {
    inviteProviderId.value = String(value ?? "");
    inviteUrl.value = "";
    inviteExpiresAt.value = "";
  };

  const createInvite = async () => {
    if (!inviteProviderId.value) {
      toast.error(t("admin.passkeySettings.selectProvider"));
      return;
    }
    await runCreateInvite(async () => {
      const provider = providers.value.find(
        (item) => item.id === inviteProviderId.value,
      );
      if (!provider) {
        throw new Error(t("admin.passkeySettings.selectProvider"));
      }
      const result = await (provider.kind === "ldap"
        ? ConfigAPI.createLdapInvite({
            totp_id: options.totpId,
            provider_id: inviteProviderId.value,
          })
        : ConfigAPI.createOIDCInvite({
            totp_id: options.totpId,
            provider_id: inviteProviderId.value,
          }));
      inviteUrl.value = result.invite_url;
      inviteExpiresAt.value = result.expires_at;
      try {
        await copyTextToClipboard(result.invite_url);
        toast.success(t("admin.passkeySettings.inviteCreatedCopied"), {
          description: result.invite_url,
        });
      } catch (error) {
        console.error("createInvite copy:", error);
        toast.warning(t("admin.passkeySettings.inviteCreatedCopyFailed"), {
          description: t("admin.passkeySettings.manualCopyHint"),
        });
      }
    });
  };

  const copyInviteUrl = async () => {
    if (!inviteUrl.value) return;
    try {
      await copyTextToClipboard(inviteUrl.value);
      toast.success(t("admin.passkeySettings.inviteCopied"), {
        description: inviteUrl.value,
      });
    } catch (error) {
      console.error("copyInviteUrl:", error);
      toast.error(t("admin.passkeySettings.copyInviteFailed"), {
        description: t("admin.passkeySettings.manualCopyHint"),
      });
    }
  };

  const deleteOidcBinding = async (bindingId: string) => {
    options.setError("");
    await runDeleteBinding(async () => {
      const binding = oidcBindings.value.find((item) => item.id === bindingId);
      if (binding?.protocol === "ldap") {
        await ConfigAPI.deleteLdapBinding(bindingId);
      } else {
        await ConfigAPI.deleteOIDCBinding(bindingId);
      }
      await loadOidcData();
      toast.success(t("admin.passkeySettings.oidcDeleted"));
    });
  };

  watch(showInviteDialog, (isOpen) => {
    if (isOpen) startAutoRefresh();
    else stopAutoRefresh();
  });
  onBeforeUnmount(stopAutoRefresh);

  return {
    copyInviteUrl,
    createInvite,
    deleteOidcBinding,
    handleInviteProviderChange,
    handleRefreshOidcBindings,
    inviteExpiresAt,
    inviteProviderId,
    inviteUrl,
    isDeletingBinding,
    isInviteCreating,
    isOidcBindingsRefreshing,
    loadOidcData,
    oidcBindings,
    openInviteDialog,
    providers,
    showInviteDialog,
  };
};
