import { computed, ref, type Ref } from "vue";
import {
  extractErrorMessage,
  useAsyncAction,
} from "@admin-shared/composables/useAsyncAction";
import { toast } from "@admin-shared/utils/toast";
import { ConfigAPI } from "../../lib/api";
import type {
  AuthAccount,
  HostMapping,
  TOTPCredential,
  TOTPSubdomainAccess,
  TOTPSubdomainAccessMode,
} from "../../types";

const BUILTIN_SELECT_PAGE_ACCESS_HOST = "__builtin_select__";
const BUILTIN_SELECT_PAGE_PATH = "/__select__";
const DEFAULT_SUBDOMAIN_ACCESS: TOTPSubdomainAccess = {
  mode: "all",
  hosts: [],
};

type Translate = (key: string, params?: Record<string, unknown>) => string;

type SubdomainAccessOption = {
  host: string;
  label: string;
  description: string;
  stale?: boolean;
  builtin?: boolean;
};

type AuthPermissionRecord = Pick<
  TOTPCredential,
  "id" | "access_scopes" | "subdomain_access"
>;

export const normalizeAuthSubdomainHost = (value: unknown) => {
  const raw = String(value ?? "")
    .trim()
    .toLowerCase();
  if (!raw) return "";
  if (
    raw === BUILTIN_SELECT_PAGE_ACCESS_HOST ||
    raw === BUILTIN_SELECT_PAGE_PATH
  ) {
    return BUILTIN_SELECT_PAGE_ACCESS_HOST;
  }

  let host = raw;
  try {
    const parsed = new URL(raw.includes("://") ? raw : `https://${raw}`);
    host = parsed.hostname;
  } catch {
    const hostCandidate =
      raw
        .replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
        .replace(/^[^@/\s]+@/, "")
        .split(/[/?#]/, 1)[0] ?? "";
    host = hostCandidate.replace(/:\d+$/, "");
  }

  host = host.trim().toLowerCase().replace(/\.+$/, "");
  if (!host || host.includes("*") || /\s/.test(host)) return "";
  return host;
};

const compareSubdomainAccessHosts = (left: string, right: string) => {
  if (left === BUILTIN_SELECT_PAGE_ACCESS_HOST) return -1;
  if (right === BUILTIN_SELECT_PAGE_ACCESS_HOST) return 1;
  return left.localeCompare(right);
};

export const normalizeAuthSubdomainAccess = (
  value: unknown,
): TOTPSubdomainAccess => {
  if (
    typeof value !== "object" ||
    value === null ||
    (value as { mode?: unknown }).mode !== "custom"
  ) {
    return { ...DEFAULT_SUBDOMAIN_ACCESS };
  }

  const hostsValue = (value as { hosts?: unknown }).hosts;
  const hosts = Array.isArray(hostsValue)
    ? [
        ...new Set(hostsValue.map(normalizeAuthSubdomainHost).filter(Boolean)),
      ].sort(compareSubdomainAccessHosts)
    : [];
  return { mode: "custom", hosts };
};

interface UseAuthSubdomainAccessOptions {
  credentials: Ref<TOTPCredential[]>;
  hostMappings: Ref<HostMapping[]>;
  replaceAuthAccount: (account: AuthAccount) => void;
  translate: Translate;
}

export function useAuthSubdomainAccess({
  credentials,
  hostMappings,
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

  const normalizeCredential = (credential: TOTPCredential): TOTPCredential => ({
    ...credential,
    access_scopes: credential.access_scopes || [],
    subdomain_access: normalizeAuthSubdomainAccess(credential.subdomain_access),
  });

  const formatSubdomainAccessHostLabel = (host: string) =>
    host === BUILTIN_SELECT_PAGE_ACCESS_HOST
      ? translate("admin.authSettings.permissionBuiltinSelectLabel")
      : host;

  const selectedSubdomainHostCount = computed(
    () => selectedSubdomainHosts.value.size,
  );

  const subdomainAccessOptions = computed<SubdomainAccessOption[]>(() => {
    const byHost = new Map<string, SubdomainAccessOption>();
    byHost.set(BUILTIN_SELECT_PAGE_ACCESS_HOST, {
      host: BUILTIN_SELECT_PAGE_ACCESS_HOST,
      label: translate("admin.authSettings.permissionBuiltinSelectLabel"),
      description: BUILTIN_SELECT_PAGE_PATH,
      builtin: true,
    });

    for (const mapping of hostMappings.value) {
      if (mapping.service_role === "auth" || mapping.use_auth !== true) {
        continue;
      }
      const host = normalizeAuthSubdomainHost(mapping.host);
      if (!host || byHost.has(host)) continue;
      const label =
        mapping.title_override.trim() || mapping.title.trim() || mapping.host;
      byHost.set(host, { host, label, description: host, stale: false });
    }

    for (const host of selectedSubdomainHosts.value) {
      if (byHost.has(host)) continue;
      byHost.set(host, {
        host,
        label: host,
        description: host,
        stale: true,
      });
    }

    const options = [...byHost.values()];
    return [
      ...options.filter((option) => option.builtin),
      ...options
        .filter((option) => !option.builtin)
        .sort((left, right) => left.host.localeCompare(right.host)),
    ];
  });

  const filteredSubdomainAccessOptions = computed(() => {
    const keyword = subdomainAccessSearch.value.trim().toLowerCase();
    if (!keyword) return subdomainAccessOptions.value;
    return subdomainAccessOptions.value.filter(
      (option) =>
        option.host.includes(keyword) ||
        option.description.toLowerCase().includes(keyword) ||
        option.label.toLowerCase().includes(keyword),
    );
  });

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
    return normalizeAuthSubdomainAccess(record.subdomain_access);
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
    const normalizedHost = normalizeAuthSubdomainHost(host);
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
            hosts: [...selectedSubdomainHosts.value].sort(
              compareSubdomainAccessHosts,
            ),
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

  const getSubdomainAccessSummary = (record: AuthPermissionRecord) => {
    const access = getSubdomainAccess(record);
    if (access.mode !== "custom") {
      return translate("admin.authSettings.permissionAll");
    }
    if (access.hosts.length === 0) {
      return translate("admin.authSettings.permissionCustomEmpty");
    }
    return translate("admin.authSettings.permissionCustomSummary", {
      count: access.hosts.length,
    });
  };

  const getSubdomainAccessPreview = (record: AuthPermissionRecord) => {
    const access = getSubdomainAccess(record);
    if (access.mode !== "custom") return "";
    if (access.hosts.length === 0) {
      return translate("admin.authSettings.permissionNoAllowedHosts");
    }
    const previewHosts = access.hosts
      .slice(0, 2)
      .map(formatSubdomainAccessHostLabel)
      .join(", ");
    if (access.hosts.length <= 2) return previewHosts;
    return translate("admin.authSettings.permissionPreviewMore", {
      hosts: previewHosts,
      count: access.hosts.length,
    });
  };

  const selectAllFilteredSubdomainHosts = () => {
    selectHosts(
      filteredSubdomainAccessOptions.value.map((option) => option.host),
    );
  };

  return {
    clearSelectedSubdomainHosts,
    closeSubdomainAccessDialog,
    editingSubdomainAccessAccount,
    editingSubdomainAccessTotp,
    filteredSubdomainAccessOptions,
    getSubdomainAccess,
    getSubdomainAccessPreview,
    getSubdomainAccessSummary,
    handleSaveSubdomainAccess,
    isSavingSubdomainAccess,
    isSubdomainAccessUpdating,
    normalizeCredential,
    openAccountSubdomainAccessDialog,
    openSubdomainAccessDialog,
    selectHosts,
    selectedSubdomainHosts,
    selectedSubdomainHostCount,
    selectAllFilteredSubdomainHosts,
    showSubdomainAccessDialog,
    subdomainAccessMode,
    subdomainAccessOptions,
    subdomainAccessSearch,
    toggleSubdomainHost,
  };
}
