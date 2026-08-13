import { ref, type Ref } from "vue";
import { extractErrorMessage } from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "@/lib/api/config";
import type { AuthAccount, TOTPCredential, TOTPAccessScope } from "../../types";

const DOCKER_ADMIN_PANEL_ACCESS_SCOPE: TOTPAccessScope = "docker_admin_panel";

type Translate = (key: string) => string;

interface AccessScopeRecord {
  id: string;
  access_scopes: TOTPAccessScope[];
}

interface UseDockerAdminAccessScopesOptions {
  credentials: Ref<TOTPCredential[]>;
  replaceAuthAccount: (account: AuthAccount) => void;
  translate: Translate;
}

export function useDockerAdminAccessScopes({
  credentials,
  replaceAuthAccount,
  translate,
}: UseDockerAdminAccessScopesOptions) {
  const updatingAccessScopeIds = ref<Set<string>>(new Set());

  function hasDockerAdminPanelAccess(record: AccessScopeRecord) {
    return (record.access_scopes || []).includes(
      DOCKER_ADMIN_PANEL_ACCESS_SCOPE,
    );
  }

  function isAccessScopeUpdating(id: string) {
    return updatingAccessScopeIds.value.has(id);
  }

  function setAccessScopeUpdating(id: string, pending: boolean) {
    const next = new Set(updatingAccessScopeIds.value);
    if (pending) {
      next.add(id);
    } else {
      next.delete(id);
    }
    updatingAccessScopeIds.value = next;
  }

  function getNextScopes(record: AccessScopeRecord, enabled: boolean) {
    const nextScopeSet = new Set<TOTPAccessScope>(record.access_scopes || []);
    if (enabled) {
      nextScopeSet.add(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
    } else {
      nextScopeSet.delete(DOCKER_ADMIN_PANEL_ACCESS_SCOPE);
    }
    return [...nextScopeSet];
  }

  function handleUpdateError(
    record: AccessScopeRecord,
    previousScopes: TOTPAccessScope[],
    error: unknown,
  ) {
    record.access_scopes = previousScopes;
    toast.error(
      extractErrorMessage(
        error,
        translate("admin.authSettings.adminPanelAccessUpdateFailed"),
      ),
    );
  }

  async function handleDockerAdminPanelAccessChange(
    totp: TOTPCredential,
    enabled: boolean,
  ) {
    const previousScopes = [...(totp.access_scopes || [])];
    const nextScopes = getNextScopes(totp, enabled);
    totp.access_scopes = nextScopes;
    setAccessScopeUpdating(totp.id, true);

    try {
      const updated = await ConfigAPI.updateTOTPAccessScopes(
        totp.id,
        nextScopes,
      );
      const target = credentials.value.find((item) => item.id === totp.id);
      if (target) {
        target.access_scopes = updated.access_scopes || [];
      }
      toast.success(translate("admin.authSettings.adminPanelAccessUpdated"));
    } catch (error) {
      handleUpdateError(totp, previousScopes, error);
    } finally {
      setAccessScopeUpdating(totp.id, false);
    }
  }

  async function handleAccountDockerAdminPanelAccessChange(
    account: AuthAccount,
    enabled: boolean,
  ) {
    const previousScopes = [...(account.access_scopes || [])];
    const nextScopes = getNextScopes(account, enabled);
    account.access_scopes = nextScopes;
    setAccessScopeUpdating(account.id, true);

    try {
      const updated = await ConfigAPI.updateAuthAccountAccessScopes(
        account.id,
        nextScopes,
      );
      replaceAuthAccount(updated);
      toast.success(translate("admin.authSettings.adminPanelAccessUpdated"));
    } catch (error) {
      handleUpdateError(account, previousScopes, error);
    } finally {
      setAccessScopeUpdating(account.id, false);
    }
  }

  return {
    handleAccountDockerAdminPanelAccessChange,
    handleDockerAdminPanelAccessChange,
    hasDockerAdminPanelAccess,
    isAccessScopeUpdating,
  };
}
