import { computed, ref, type Ref } from "vue";
import { useI18n } from "vue-i18n";
import { toast } from "@admin-shared/utils/toast";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { ConfigAPI } from "@/lib/api/config";
import type { AuthAccount, TOTPSubdomainAccess } from "@/types";

export const useAuthAccountWorkflow = ({
  authAccounts,
  normalizeSubdomainAccess,
  refreshAuthModePreview,
  showAuthModeSwitchDialog,
}: {
  authAccounts: Ref<AuthAccount[]>;
  normalizeSubdomainAccess: (value: unknown) => TOTPSubdomainAccess;
  refreshAuthModePreview: () => Promise<void>;
  showAuthModeSwitchDialog: Ref<boolean>;
}) => {
  const { t } = useI18n();
  const showAuthAccountDialog = ref(false);
  const showAccountPasswordDialog = ref(false);
  const editingAuthAccount = ref<AuthAccount | null>(null);
  const authAccountUsernameInput = ref("");
  const isCreatingAuthAccount = ref(false);
  const editingPasswordAccount = ref<AuthAccount | null>(null);
  const accountPasswordUsernameInput = ref("");
  const accountPasswordInput = ref("");
  const isAccountPasswordVisible = ref(false);
  const reopenAuthModeSwitchAfterPasswordSave = ref(false);

  const { isPending: isSavingAccountPassword, run: runSaveAccountPassword } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            t("admin.authSettings.accountPasswordSaveFailed"),
          ),
        );
      },
    });
  const { isPending: isSavingAuthAccount, run: runSaveAuthAccount } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(error, t("admin.authSettings.accountSaveFailed")),
        );
      },
    });

  const isAccountPasswordSetupMode = computed(
    () =>
      isCreatingAuthAccount.value ||
      editingPasswordAccount.value?.passwordConfigured === false,
  );

  const accountPasswordDialogTitle = computed(() => {
    if (isCreatingAuthAccount.value) {
      return t("admin.authSettings.createAccount");
    }
    if (isAccountPasswordSetupMode.value) {
      return t("admin.authSettings.setupAccountPassword");
    }
    return editingPasswordAccount.value?.passwordConfigured
      ? t("admin.authSettings.changePassword")
      : t("admin.authSettings.setPassword");
  });

  const accountPasswordDialogDescription = computed(() => {
    if (isCreatingAuthAccount.value) {
      return t("admin.authSettings.createAccountDescription");
    }
    if (isAccountPasswordSetupMode.value) {
      return t("admin.authSettings.setupAccountPasswordDescription");
    }
    return t("admin.authSettings.accountPasswordDescription", {
      username: editingPasswordAccount.value?.username || "",
    });
  });

  const normalizeAuthAccount = (account: AuthAccount): AuthAccount => ({
    ...account,
    displayName: account.username,
    access_scopes: account.access_scopes || [],
    subdomain_access: normalizeSubdomainAccess(account.subdomain_access),
    passwordConfigured: account.passwordConfigured === true,
    totpConfigured: account.totpConfigured === true,
  });

  const replaceAuthAccount = (account: AuthAccount) => {
    const normalized = normalizeAuthAccount(account);
    const index = authAccounts.value.findIndex(
      (item) => item.id === account.id,
    );
    if (index >= 0) {
      authAccounts.value.splice(index, 1, normalized);
      return;
    }
    authAccounts.value.push(normalized);
  };

  const validateAccountUsername = (value: string, account: AuthAccount) => {
    const username = value.trim();
    if (!username) {
      return t("admin.authSettings.accountUsernameRequired");
    }
    const isDuplicate = authAccounts.value.some(
      (item) => item.id !== account.id && item.username === username,
    );
    if (isDuplicate) {
      return t("admin.authSettings.accountUsernameDuplicate");
    }
  };

  const usernameSecurityWarning = (value: string) => {
    const username = value.trim();
    return username && username.length < 3
      ? t("admin.authSettings.shortUsernameWarning")
      : "";
  };

  const passwordSecurityWarning = (password: string) => {
    if (!password) return "";
    const hasLetters = /[A-Za-z]/.test(password);
    const hasNumbers = /\d/.test(password);
    return password.length < 6 ||
      /\s/.test(password) ||
      !hasLetters ||
      !hasNumbers
      ? t("admin.authSettings.weakPasswordWarning")
      : "";
  };

  const openAuthAccountDialog = (account: AuthAccount) => {
    editingAuthAccount.value = account;
    authAccountUsernameInput.value = account.username;
    showAuthAccountDialog.value = true;
  };

  const closeAuthAccountDialog = () => {
    showAuthAccountDialog.value = false;
    editingAuthAccount.value = null;
    authAccountUsernameInput.value = "";
  };

  const openCreateAuthAccountDialog = () => {
    isCreatingAuthAccount.value = true;
    editingPasswordAccount.value = null;
    reopenAuthModeSwitchAfterPasswordSave.value = false;
    accountPasswordUsernameInput.value = "";
    accountPasswordInput.value = "";
    isAccountPasswordVisible.value = false;
    showAccountPasswordDialog.value = true;
  };

  const handleSaveAuthAccount = async () => {
    const account = editingAuthAccount.value;
    if (!account) return;
    const username = authAccountUsernameInput.value.trim();
    const validationMessage = validateAccountUsername(username, account);
    if (validationMessage) {
      toast.error(validationMessage);
      return;
    }
    await runSaveAuthAccount(async () => {
      const updated = await ConfigAPI.updateAuthAccount(account.id, {
        username,
      });
      replaceAuthAccount(updated);
      closeAuthAccountDialog();
      if (showAuthModeSwitchDialog.value) {
        await refreshAuthModePreview();
      }
      toast.success(t("admin.authSettings.accountSaved"));
    });
  };

  const saveAccountUsername = async (account: AuthAccount, value: string) => {
    const username = value.trim();
    try {
      const updated = await ConfigAPI.updateAuthAccount(account.id, {
        username,
      });
      replaceAuthAccount(updated);
      if (showAuthModeSwitchDialog.value) {
        await refreshAuthModePreview();
      }
      toast.success(t("admin.authSettings.accountSaved"));
    } catch (error) {
      throw new Error(
        extractErrorMessage(error, t("admin.authSettings.accountSaveFailed")),
        { cause: error },
      );
    }
  };

  const openAccountPasswordDialog = (
    account: AuthAccount,
    reopenSwitchAfterSave = false,
  ) => {
    isCreatingAuthAccount.value = false;
    editingPasswordAccount.value = account;
    reopenAuthModeSwitchAfterPasswordSave.value = reopenSwitchAfterSave;
    accountPasswordUsernameInput.value = account.username;
    accountPasswordInput.value = "";
    isAccountPasswordVisible.value = false;
    showAccountPasswordDialog.value = true;
  };

  const openAccountPasswordDialogFromSwitch = (account: AuthAccount) => {
    showAuthModeSwitchDialog.value = false;
    openAccountPasswordDialog(account, true);
  };

  const closeAccountPasswordDialog = () => {
    showAccountPasswordDialog.value = false;
    isCreatingAuthAccount.value = false;
    editingPasswordAccount.value = null;
    reopenAuthModeSwitchAfterPasswordSave.value = false;
    accountPasswordUsernameInput.value = "";
    accountPasswordInput.value = "";
    isAccountPasswordVisible.value = false;
  };

  const handleSaveAccountPassword = async () => {
    const account = editingPasswordAccount.value;
    if (!isCreatingAuthAccount.value && !account) return;
    const password = accountPasswordInput.value;
    const username = accountPasswordUsernameInput.value.trim();
    if (isAccountPasswordSetupMode.value && !username) {
      toast.error(t("admin.authSettings.accountUsernameRequired"));
      return;
    }
    if (!password) {
      toast.error(t("admin.authSettings.accountPasswordRequired"));
      return;
    }

    await runSaveAccountPassword(async () => {
      let updated: AuthAccount | null = null;
      const wasCreating = isCreatingAuthAccount.value;
      if (isCreatingAuthAccount.value) {
        updated = await ConfigAPI.createAuthAccount({ username, password });
      } else if (isAccountPasswordSetupMode.value && account) {
        updated = await ConfigAPI.setupAuthAccount(account.id, {
          username,
          password,
        });
      } else if (account) {
        updated = await ConfigAPI.setAuthAccountPassword(account.id, password);
      }
      if (!updated) return;

      const shouldReopenSwitch = reopenAuthModeSwitchAfterPasswordSave.value;
      replaceAuthAccount(updated);
      closeAccountPasswordDialog();
      if (shouldReopenSwitch) {
        showAuthModeSwitchDialog.value = true;
        await refreshAuthModePreview();
      } else if (showAuthModeSwitchDialog.value) {
        await refreshAuthModePreview();
      }
      toast.success(
        t(
          wasCreating
            ? "admin.authSettings.accountCreated"
            : "admin.authSettings.accountPasswordSaved",
        ),
      );
    });
  };

  return {
    accountPasswordDialogDescription,
    accountPasswordDialogTitle,
    accountPasswordInput,
    accountPasswordUsernameInput,
    authAccountUsernameInput,
    closeAccountPasswordDialog,
    closeAuthAccountDialog,
    editingAuthAccount,
    editingPasswordAccount,
    handleSaveAccountPassword,
    handleSaveAuthAccount,
    isAccountPasswordSetupMode,
    isAccountPasswordVisible,
    isCreatingAuthAccount,
    isSavingAccountPassword,
    isSavingAuthAccount,
    normalizeAuthAccount,
    openAccountPasswordDialog,
    openAccountPasswordDialogFromSwitch,
    openAuthAccountDialog,
    openCreateAuthAccountDialog,
    passwordSecurityWarning,
    replaceAuthAccount,
    saveAccountUsername,
    showAccountPasswordDialog,
    showAuthAccountDialog,
    usernameSecurityWarning,
    validateAccountUsername,
  };
};
