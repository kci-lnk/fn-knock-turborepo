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
  StreamMapping,
  TOTPCredential,
  TOTPSubdomainAccess,
  TOTPSubdomainAccessMode,
  TOTPStreamAccess,
} from "../../types";

const BUILTIN_SELECT_PAGE_ACCESS_HOST = "__builtin_select__";
const BUILTIN_SELECT_PAGE_PATH = "/__select__";
const BUILTIN_WOL_PAGE_ACCESS_HOST = "__builtin_wol__";
const BUILTIN_WOL_PAGE_PATH = "/__wol__";
const DEFAULT_SUBDOMAIN_ACCESS: TOTPSubdomainAccess = {
  mode: "all",
  hosts: [],
  streams: [],
};
const HOST_ACCESS_KEY_PREFIX = "host:";
const STREAM_ACCESS_KEY_PREFIX = "stream:";

type Translate = (key: string, params?: Record<string, unknown>) => string;

type SubdomainAccessOption = {
  key: string;
  kind: "host" | "stream";
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
  if (raw === BUILTIN_WOL_PAGE_ACCESS_HOST || raw === BUILTIN_WOL_PAGE_PATH) {
    return BUILTIN_WOL_PAGE_ACCESS_HOST;
  }

  let host: string;
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
  if (left === BUILTIN_WOL_PAGE_ACCESS_HOST) return -1;
  if (right === BUILTIN_WOL_PAGE_ACCESS_HOST) return 1;
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
  const streamsValue = (value as { streams?: unknown }).streams;
  const streams = Array.isArray(streamsValue)
    ? [
        ...new Map(
          streamsValue
            .map(normalizeAuthStreamAccess)
            .filter((stream): stream is TOTPStreamAccess => stream !== null)
            .map((stream) => [createAuthStreamAccessKey(stream), stream]),
        ).values(),
      ].sort(compareAuthStreamAccess)
    : [];
  return { mode: "custom", hosts, streams };
};

export const normalizeAuthStreamAccess = (
  value: unknown,
): TOTPStreamAccess | null => {
  if (typeof value !== "object" || value === null) return null;
  const rawProtocol = String((value as { protocol?: unknown }).protocol ?? "")
    .trim()
    .toLowerCase();
  const protocol =
    rawProtocol === "udp" ? "udp" : rawProtocol === "tcp" ? "tcp" : null;
  const listenPort = Number((value as { listen_port?: unknown }).listen_port);
  if (
    protocol === null ||
    !Number.isInteger(listenPort) ||
    listenPort < 1 ||
    listenPort > 65535
  ) {
    return null;
  }
  return { protocol, listen_port: listenPort };
};

export const createAuthStreamAccessKey = (stream: TOTPStreamAccess) =>
  `${STREAM_ACCESS_KEY_PREFIX}${stream.protocol}:${stream.listen_port}`;

const createAuthHostAccessKey = (host: string) =>
  `${HOST_ACCESS_KEY_PREFIX}${host}`;

const compareAuthStreamAccess = (
  left: TOTPStreamAccess,
  right: TOTPStreamAccess,
) =>
  left.listen_port === right.listen_port
    ? left.protocol.localeCompare(right.protocol)
    : left.listen_port - right.listen_port;

const parseAuthStreamAccessKey = (key: string): TOTPStreamAccess | null => {
  if (!key.startsWith(STREAM_ACCESS_KEY_PREFIX)) return null;
  const [protocol, rawPort, extra] = key
    .slice(STREAM_ACCESS_KEY_PREFIX.length)
    .split(":");
  if (extra !== undefined) return null;
  return normalizeAuthStreamAccess({
    protocol,
    listen_port: Number(rawPort),
  });
};

interface UseAuthSubdomainAccessOptions {
  credentials: Ref<TOTPCredential[]>;
  hostMappings: Ref<HostMapping[]>;
  streamMappings: Ref<StreamMapping[]>;
  wolFeatureEnabled: Ref<boolean>;
  replaceAuthAccount: (account: AuthAccount) => void;
  translate: Translate;
}

export function useAuthSubdomainAccess({
  credentials,
  hostMappings,
  streamMappings,
  wolFeatureEnabled,
  replaceAuthAccount,
  translate,
}: UseAuthSubdomainAccessOptions) {
  const showSubdomainAccessDialog = ref(false);
  const editingSubdomainAccessTotp = ref<TOTPCredential | null>(null);
  const editingSubdomainAccessAccount = ref<AuthAccount | null>(null);
  const subdomainAccessMode = ref<TOTPSubdomainAccessMode>("all");
  const selectedAccessKeys = ref<Set<string>>(new Set());
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
      : host === BUILTIN_WOL_PAGE_ACCESS_HOST
        ? translate("admin.authSettings.permissionBuiltinWolLabel")
        : host;

  const selectedAccessCount = computed(() => selectedAccessKeys.value.size);

  const subdomainAccessOptions = computed<SubdomainAccessOption[]>(() => {
    const byHost = new Map<string, SubdomainAccessOption>();
    byHost.set(createAuthHostAccessKey(BUILTIN_SELECT_PAGE_ACCESS_HOST), {
      key: createAuthHostAccessKey(BUILTIN_SELECT_PAGE_ACCESS_HOST),
      kind: "host",
      label: translate("admin.authSettings.permissionBuiltinSelectLabel"),
      description: BUILTIN_SELECT_PAGE_PATH,
      builtin: true,
    });
    if (wolFeatureEnabled.value) {
      byHost.set(createAuthHostAccessKey(BUILTIN_WOL_PAGE_ACCESS_HOST), {
        key: createAuthHostAccessKey(BUILTIN_WOL_PAGE_ACCESS_HOST),
        kind: "host",
        label: translate("admin.authSettings.permissionBuiltinWolLabel"),
        description: BUILTIN_WOL_PAGE_PATH,
        builtin: true,
      });
    }

    for (const mapping of hostMappings.value) {
      if (mapping.service_role === "auth" || mapping.use_auth !== true) {
        continue;
      }
      const host = normalizeAuthSubdomainHost(mapping.host);
      const key = createAuthHostAccessKey(host);
      if (!host || byHost.has(key)) continue;
      const label =
        mapping.title_override.trim() || mapping.title.trim() || mapping.host;
      byHost.set(key, {
        key,
        kind: "host",
        label,
        description: host,
        stale: false,
      });
    }

    for (const mapping of streamMappings.value) {
      if (mapping.use_auth !== true) continue;
      const stream = normalizeAuthStreamAccess(mapping);
      if (!stream) continue;
      const key = createAuthStreamAccessKey(stream);
      byHost.set(key, {
        key,
        kind: "stream",
        label: `${stream.protocol.toUpperCase()}/${stream.listen_port}`,
        description: mapping.target,
        stale: false,
      });
    }

    for (const key of selectedAccessKeys.value) {
      if (byHost.has(key)) continue;
      const stream = parseAuthStreamAccessKey(key);
      const host = key.startsWith(HOST_ACCESS_KEY_PREFIX)
        ? key.slice(HOST_ACCESS_KEY_PREFIX.length)
        : "";
      if (!stream && !host) continue;
      byHost.set(key, {
        key,
        kind: stream ? "stream" : "host",
        label: stream
          ? `${stream.protocol.toUpperCase()}/${stream.listen_port}`
          : formatSubdomainAccessHostLabel(host),
        description: stream
          ? translate("admin.authSettings.permissionMissingStream")
          : host,
        stale: true,
      });
    }

    const options = [...byHost.values()];
    return [
      ...options.filter((option) => option.builtin),
      ...options
        .filter((option) => !option.builtin)
        .sort((left, right) => {
          if (left.kind !== right.kind) return left.kind === "host" ? -1 : 1;
          return left.key.localeCompare(right.key);
        }),
    ];
  });

  const filteredSubdomainAccessOptions = computed(() => {
    const keyword = subdomainAccessSearch.value.trim().toLowerCase();
    if (!keyword) return subdomainAccessOptions.value;
    return subdomainAccessOptions.value.filter(
      (option) =>
        option.key.includes(keyword) ||
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
    selectedAccessKeys.value = new Set([
      ...access.hosts.map(createAuthHostAccessKey),
      ...access.streams.map(createAuthStreamAccessKey),
    ]);
    subdomainAccessSearch.value = "";
    showSubdomainAccessDialog.value = true;
  }

  function openAccountSubdomainAccessDialog(account: AuthAccount) {
    const access = getSubdomainAccess(account);
    editingSubdomainAccessTotp.value = null;
    editingSubdomainAccessAccount.value = account;
    subdomainAccessMode.value = access.mode;
    selectedAccessKeys.value = new Set([
      ...access.hosts.map(createAuthHostAccessKey),
      ...access.streams.map(createAuthStreamAccessKey),
    ]);
    subdomainAccessSearch.value = "";
    showSubdomainAccessDialog.value = true;
  }

  function closeSubdomainAccessDialog() {
    showSubdomainAccessDialog.value = false;
    editingSubdomainAccessTotp.value = null;
    editingSubdomainAccessAccount.value = null;
    subdomainAccessMode.value = "all";
    selectedAccessKeys.value = new Set();
    subdomainAccessSearch.value = "";
  }

  function toggleAccessOption(key: string, checked: boolean) {
    if (
      !key.startsWith(HOST_ACCESS_KEY_PREFIX) &&
      !key.startsWith(STREAM_ACCESS_KEY_PREFIX)
    ) {
      return;
    }
    const next = new Set(selectedAccessKeys.value);
    if (checked) {
      next.add(key);
    } else {
      next.delete(key);
    }
    selectedAccessKeys.value = next;
  }

  function selectAccessOptions(keys: Iterable<string>) {
    const next = new Set(selectedAccessKeys.value);
    for (const key of keys) {
      next.add(key);
    }
    selectedAccessKeys.value = next;
  }

  function clearSelectedAccessOptions() {
    selectedAccessKeys.value = new Set();
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
            hosts: [...selectedAccessKeys.value]
              .filter((key) => key.startsWith(HOST_ACCESS_KEY_PREFIX))
              .map((key) =>
                normalizeAuthSubdomainHost(
                  key.slice(HOST_ACCESS_KEY_PREFIX.length),
                ),
              )
              .filter(Boolean)
              .sort(compareSubdomainAccessHosts),
            streams: [...selectedAccessKeys.value]
              .map(parseAuthStreamAccessKey)
              .filter((stream): stream is TOTPStreamAccess => stream !== null)
              .sort(compareAuthStreamAccess),
          }
        : { mode: "all", hosts: [], streams: [] };

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
    if (access.hosts.length + access.streams.length === 0) {
      return translate("admin.authSettings.permissionCustomEmpty");
    }
    return translate("admin.authSettings.permissionCustomSummary", {
      count: access.hosts.length + access.streams.length,
    });
  };

  const getSubdomainAccessPreview = (record: AuthPermissionRecord) => {
    const access = getSubdomainAccess(record);
    if (access.mode !== "custom") return "";
    const labels = [
      ...access.hosts.map(formatSubdomainAccessHostLabel),
      ...access.streams.map(
        (stream) => `${stream.protocol.toUpperCase()}/${stream.listen_port}`,
      ),
    ];
    if (labels.length === 0) {
      return translate("admin.authSettings.permissionNoAllowedHosts");
    }
    const previewHosts = labels.slice(0, 2).join(", ");
    if (labels.length <= 2) return previewHosts;
    return translate("admin.authSettings.permissionPreviewMore", {
      hosts: previewHosts,
      count: labels.length,
    });
  };

  const selectAllFilteredAccessOptions = () => {
    selectAccessOptions(
      filteredSubdomainAccessOptions.value.map((option) => option.key),
    );
  };

  return {
    clearSelectedAccessOptions,
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
    selectedAccessCount,
    selectedAccessKeys,
    selectAllFilteredAccessOptions,
    showSubdomainAccessDialog,
    subdomainAccessMode,
    subdomainAccessOptions,
    subdomainAccessSearch,
    toggleAccessOption,
  };
}
