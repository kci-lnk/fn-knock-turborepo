import { ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type {
  AuthAccount,
  TOTPCredential,
  TOTPSubdomainAccess,
  TOTPSubdomainAccessMode,
} from "../../types";

type Translate = (key: string) => string;

interface UseAuthSubdomainAccessOptions {
  compareHosts: (left: string, right: string) => number;
  credentials: Ref<TOTPCredential[]>;
  normalizeAccess: (value: unknown) => TOTPSubdomainAccess;
  normalizeCredential: (credential: TOTPCredential) => TOTPCredential;
  normalizeHost: (host: string) => string;
  replaceAuthAccount: (account: AuthAccount) => void;
  translate: Translate;
}

export function useAuthSubdomainAccess({
  compareHosts,
  credentials,
  normalizeAccess,
  normalizeCredential,
  normalizeHost,
  replaceAuthAccount,
  translate,
}: UseAuthSubdomainAccessOptions) {
  const showSubdomainAccessDialog = ref(false);
  const editingSubdomainAccessTotp = ref<TOTPCredential | null>(null);
  const editingSubdomainAccessAccount = ref<AuthAccount | null>(null);
  const subdomainAccessMode = ref<TOTPSubdomainAccessMode>("all");
  const selectedSubdomainHosts = ref<Set<string>>(new Set());
  const subdomainAccessSearch = ref("");
  const updatingSubdomainAccessIds = ref<Set<string>>(new Set());

  const { isPending: isSavingSubdomainAccess, run: runSaveSubdomainAccess } =
    useAsyncAction({
      onError: (error) => {
        toast.error(
          extractErrorMessage(
            error,
            translate("admin.authSettings.permissionUpdateFailed"),
          ),
        );
      },
    });

  function getSubdomainAccess(
    record: Pick<TOTPCredential | AuthAccount, "subdomain_access">,
  ) {
    return normalizeAccess(record.subdomain_access);
  }

  function openSubdomainAccessDialog(totp: TOTPCredential) {
    const access = getSubdomainAccess(totp);
    editingSubdomainAccessTotp.value = totp;
    editingSubdomainAccessAccount.value = null;
    subdomainAccessMode.value = access.mode;
    selectedSubdomainHosts.value = new Set(access.hosts);
    subdomainAccessSearch.value = "";
    showSubdomainAccessDialog.value = true;
  }

  function openAccountSubdomainAccessDialog(account: AuthAccount) {
    const access = getSubdomainAccess(account);
    editingSubdomainAccessTotp.value = null;
    editingSubdomainAccessAccount.value = account;
    subdomainAccessMode.value = access.mode;
    selectedSubdomainHosts.value = new Set(access.hosts);
    subdomainAccessSearch.value = "";
    showSubdomainAccessDialog.value = true;
  }

  function closeSubdomainAccessDialog() {
    showSubdomainAccessDialog.value = false;
    editingSubdomainAccessTotp.value = null;
    editingSubdomainAccessAccount.value = null;
    subdomainAccessMode.value = "all";
    selectedSubdomainHosts.value = new Set();
    subdomainAccessSearch.value = "";
  }

  function toggleSubdomainHost(host: string, checked: boolean) {
    const normalizedHost = normalizeHost(host);
    if (!normalizedHost) return;

    const next = new Set(selectedSubdomainHosts.value);
    if (checked) {
      next.add(normalizedHost);
    } else {
      next.delete(normalizedHost);
    }
    selectedSubdomainHosts.value = next;
  }

  function selectHosts(hosts: Iterable<string>) {
    const next = new Set(selectedSubdomainHosts.value);
    for (const host of hosts) {
      next.add(host);
    }
    selectedSubdomainHosts.value = next;
  }

  function clearSelectedSubdomainHosts() {
    selectedSubdomainHosts.value = new Set();
  }

  function isSubdomainAccessUpdating(id: string) {
    return updatingSubdomainAccessIds.value.has(id);
  }

  function setSubdomainAccessUpdating(id: string, pending: boolean) {
    const next = new Set(updatingSubdomainAccessIds.value);
    if (pending) {
      next.add(id);
    } else {
      next.delete(id);
    }
    updatingSubdomainAccessIds.value = next;
  }

  async function handleSaveSubdomainAccess() {
    const target =
      editingSubdomainAccessAccount.value || editingSubdomainAccessTotp.value;
    if (!target) return;

    const subdomainAccess: TOTPSubdomainAccess =
      subdomainAccessMode.value === "custom"
        ? {
            mode: "custom",
            hosts: [...selectedSubdomainHosts.value].sort(compareHosts),
          }
        : { mode: "all", hosts: [] };

    setSubdomainAccessUpdating(target.id, true);
    try {
      await runSaveSubdomainAccess(async () => {
        if (editingSubdomainAccessAccount.value) {
          const updated = await ConfigAPI.updateAuthAccountSubdomainAccess(
            target.id,
            subdomainAccess,
          );
          replaceAuthAccount(updated);
        } else {
          const updated = normalizeCredential(
            await ConfigAPI.updateTOTPSubdomainAccess(
              target.id,
              subdomainAccess,
            ),
          );
          const existing = credentials.value.find(
            (item) => item.id === target.id,
          );
          if (existing) {
            Object.assign(existing, updated);
          }
        }
        toast.success(translate("admin.authSettings.permissionUpdated"));
        closeSubdomainAccessDialog();
      });
    } finally {
      setSubdomainAccessUpdating(target.id, false);
    }
  }

  return {
    clearSelectedSubdomainHosts,
    closeSubdomainAccessDialog,
    editingSubdomainAccessAccount,
    editingSubdomainAccessTotp,
    getSubdomainAccess,
    handleSaveSubdomainAccess,
    isSavingSubdomainAccess,
    isSubdomainAccessUpdating,
    openAccountSubdomainAccessDialog,
    openSubdomainAccessDialog,
    selectHosts,
    selectedSubdomainHosts,
    showSubdomainAccessDialog,
    subdomainAccessMode,
    subdomainAccessSearch,
    toggleSubdomainHost,
  };
}
