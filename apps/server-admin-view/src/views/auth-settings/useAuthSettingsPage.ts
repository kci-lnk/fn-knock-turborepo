import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { useMediaQueryMatch } from "@admin-shared/composables/useMediaQueryMatch";
import { isProtectedAdminPanelDeploymentTarget } from "../../lib/admin-panel-runtime";
import { useConfigStore } from "../../store/config";
import { useDockerAdminAuthStore } from "../../store/dockerAdminAuth";
import type {
  AuthAccount,
  AuthLoginMode,
  AuthLoginModeStatus,
  HostMapping,
  StreamMapping,
  TOTPCredential,
} from "../../types";
import { useAuthAccountWorkflow } from "./useAuthAccountWorkflow";
import { useAuthCredentialTransfer } from "./useAuthCredentialTransfer";
import { useAuthModeSwitch } from "./useAuthModeSwitch";
import { useAuthSettingsResource } from "./useAuthSettingsResource";
import {
  normalizeAuthSubdomainAccess,
  useAuthSubdomainAccess,
} from "./useAuthSubdomainAccess";
import { useDockerAdminAccessScopes } from "./useDockerAdminAccessScopes";
import { useTotpSetupWorkflow } from "./useTotpSetupWorkflow";

export function useAuthSettingsPage() {
  const { t } = useI18n();
  const router = useRouter();
  const dockerAdminAuthStore = useDockerAdminAuthStore();
  const configStore = useConfigStore();
  const credentials = ref<TOTPCredential[]>([]);
  const authAccounts = ref<AuthAccount[]>([]);
  const authLoginMode = ref<AuthLoginMode>("totp");
  const authModeStatus = ref<AuthLoginModeStatus | null>(null);
  const hostMappings = ref<HostMapping[]>([]);
  const streamMappings = ref<StreamMapping[]>([]);
  const openAdminPanelAccessTooltipId = ref<string | null>(null);
  const isTouchInteraction = useMediaQueryMatch(
    "(hover: none), (pointer: coarse)",
  );
  const wolFeatureEnabled = computed(
    () => configStore.config?.wol_feature?.enabled === true,
  );

  const authModeController = useAuthModeSwitch({
    authLoginMode,
    authModeStatus,
    refreshStatus: fetchStatus,
    translate: (key) => t(key),
  });
  const accountController = useAuthAccountWorkflow({
    authAccounts,
    normalizeSubdomainAccess: normalizeAuthSubdomainAccess,
    refreshAuthModePreview: authModeController.refreshAuthModePreview,
    showAuthModeSwitchDialog: authModeController.showAuthModeSwitchDialog,
  });
  const subdomainAccessController = useAuthSubdomainAccess({
    credentials,
    hostMappings,
    streamMappings,
    wolFeatureEnabled,
    replaceAuthAccount: accountController.replaceAuthAccount,
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const dockerAccessController = useDockerAdminAccessScopes({
    credentials,
    replaceAuthAccount: accountController.replaceAuthAccount,
    translate: (key) => t(key),
  });
  const totpController = useTotpSetupWorkflow({
    credentials,
    onReopenAuthModeSwitch: async () => {
      authModeController.showAuthModeSwitchDialog.value = true;
      await authModeController.refreshAuthModePreview();
    },
    refreshStatus: fetchStatus,
    replaceAuthAccount: accountController.replaceAuthAccount,
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const credentialTransferController = useAuthCredentialTransfer({
    authAccounts,
    authLoginMode,
    credentials,
    refreshStatus: fetchStatus,
    translate: (key, params) => (params ? t(key, params) : t(key)),
  });
  const authSettingsResource = useAuthSettingsResource({
    authAccounts,
    authLoginMode,
    authModeStatus,
    credentials,
    hostMappings,
    streamMappings,
    normalizeAuthAccount: accountController.normalizeAuthAccount,
    normalizeCredential: subdomainAccessController.normalizeCredential,
    translate: (key) => t(key),
  });

  const showAdminPanelAccessColumn = computed(() =>
    isProtectedAdminPanelDeploymentTarget(
      dockerAdminAuthStore.state?.deployment_target,
    ),
  );
  const totpTableClass = computed(() =>
    showAdminPanelAccessColumn.value
      ? "min-w-[920px] table-fixed"
      : "min-w-[780px] table-fixed",
  );
  const totpTableColspan = computed(() =>
    showAdminPanelAccessColumn.value ? 6 : 5,
  );
  const authAccountTableClass = computed(() =>
    showAdminPanelAccessColumn.value
      ? "min-w-[840px] table-fixed"
      : "min-w-[680px] table-fixed",
  );
  const authAccountTableColspan = computed(() =>
    showAdminPanelAccessColumn.value ? 4 : 3,
  );
  const authSettingsTitle = computed(() =>
    authLoginMode.value === "password"
      ? t("admin.authSettings.passwordAccountsTitle")
      : t("admin.authSettings.title"),
  );
  const authSettingsDescription = computed(() =>
    authLoginMode.value === "password"
      ? t("admin.authSettings.passwordAccountsDescription")
      : t("admin.authSettings.description"),
  );
  const primaryAuthActionLabel = computed(() =>
    authLoginMode.value === "password"
      ? t("admin.authSettings.createAccount")
      : t("admin.authSettings.bindNewToken"),
  );

  async function fetchStatus() {
    await authSettingsResource.fetchStatus();
  }

  const handlePrimaryAuthAction = () => {
    if (authLoginMode.value === "password") {
      accountController.openCreateAuthAccountDialog();
      return;
    }
    void totpController.openSetupDialog();
  };
  const isAdminPanelAccessTooltipOpen = (totpId: string) =>
    openAdminPanelAccessTooltipId.value === totpId;
  const handleAdminPanelAccessTooltipOpenChange = (
    totpId: string,
    nextOpen: boolean,
  ) => {
    openAdminPanelAccessTooltipId.value = nextOpen ? totpId : null;
  };
  const handleAdminPanelAccessTooltipClick = (totpId: string) => {
    if (!isTouchInteraction.value) return;
    openAdminPanelAccessTooltipId.value =
      openAdminPanelAccessTooltipId.value === totpId ? null : totpId;
  };
  const openAccountTotpSetupDialogFromSwitch = async (
    account: AuthAccount,
  ) => {
    authModeController.showAuthModeSwitchDialog.value = false;
    await totpController.openAccountTotpSetupDialog(account, true);
  };
  const validateComment = (newText: string, id: string) => {
    if (
      credentials.value.some(
        (credential) =>
          credential.comment === newText && credential.id !== id,
      )
    ) {
      return t("admin.authSettings.commentDuplicate");
    }
  };
  const goToPasskeys = (totpId: string) => {
    void router.push(`/auth/passkeys/${encodeURIComponent(totpId)}`);
  };
  const goToOidcProviders = () => {
    void router.push("/auth/external-providers");
  };

  return {
    ...accountController,
    ...authModeController,
    ...authSettingsResource,
    ...credentialTransferController,
    ...dockerAccessController,
    ...subdomainAccessController,
    ...totpController,
    authAccountTableClass,
    authAccountTableColspan,
    authAccounts,
    authLoginMode,
    authSettingsDescription,
    authSettingsTitle,
    credentials,
    goToOidcProviders,
    goToPasskeys,
    handleAdminPanelAccessTooltipClick,
    handleAdminPanelAccessTooltipOpenChange,
    handlePrimaryAuthAction,
    isAdminPanelAccessTooltipOpen,
    openAccountTotpSetupDialogFromSwitch,
    primaryAuthActionLabel,
    showAdminPanelAccessColumn,
    totpTableClass,
    totpTableColspan,
    validateComment,
  };
}

export type AuthSettingsPageController = ReturnType<
  typeof useAuthSettingsPage
>;
