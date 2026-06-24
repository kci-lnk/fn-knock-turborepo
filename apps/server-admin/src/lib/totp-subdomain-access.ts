export type TOTPSubdomainAccessMode = "all" | "custom";

export type TOTPSubdomainAccess = {
  mode: TOTPSubdomainAccessMode;
  hosts: string[];
};

export const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE = "__builtin_select__";
export const TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH = "/__select__";

export const DEFAULT_TOTP_SUBDOMAIN_ACCESS: TOTPSubdomainAccess = {
  mode: "all",
  hosts: [],
};

const stripHostPort = (value: string): string => {
  try {
    return new URL(`https://${value}`).hostname;
  } catch {
    return value.replace(/:\d+$/, "");
  }
};

export const normalizeSubdomainAccessHost = (
  value: unknown,
): string => {
  let host = String(value ?? "").trim().toLowerCase();
  if (!host) return "";
  if (
    host === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE ||
    host === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE_PATH
  ) {
    return TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE;
  }

  try {
    const url = host.includes("://")
      ? new URL(host)
      : new URL(`https://${host}`);
    host = url.hostname;
  } catch {
    host = host
      .replace(/^[a-z][a-z0-9+.-]*:\/\//i, "")
      .replace(/^[^@/]+@/, "")
      .split(/[/?#]/)[0]!
      .trim();
    host = stripHostPort(host);
  }

  host = host.trim().toLowerCase().replace(/\.+$/, "");
  if (!host || host.includes("*") || /[\s,]/.test(host)) return "";
  return host;
};

export const normalizeSubdomainAccessHosts = (
  value: unknown,
): string[] => {
  if (!Array.isArray(value)) return [];

  const hosts = new Set<string>();
  for (const item of value) {
    const host = normalizeSubdomainAccessHost(item);
    if (host) hosts.add(host);
  }
  return [...hosts].sort((left, right) => {
    if (left === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE) return -1;
    if (right === TOTP_SUBDOMAIN_ACCESS_SELECT_PAGE) return 1;
    return left.localeCompare(right);
  });
};

export const normalizeTotpSubdomainAccess = (
  value: unknown,
): TOTPSubdomainAccess => {
  if (!value || typeof value !== "object") {
    return { ...DEFAULT_TOTP_SUBDOMAIN_ACCESS };
  }

  const raw = value as Partial<TOTPSubdomainAccess>;
  if (raw.mode !== "custom") {
    return { ...DEFAULT_TOTP_SUBDOMAIN_ACCESS };
  }

  return {
    mode: "custom",
    hosts: normalizeSubdomainAccessHosts(raw.hosts),
  };
};

export const isTotpSubdomainAccessRestricted = (
  access: unknown,
): boolean => normalizeTotpSubdomainAccess(access).mode === "custom";

export const isHostAllowedByTotpSubdomainAccess = ({
  access,
  host,
}: {
  access: unknown;
  host: unknown;
}): boolean => {
  const normalizedAccess = normalizeTotpSubdomainAccess(access);
  if (normalizedAccess.mode !== "custom") return true;

  const normalizedHost = normalizeSubdomainAccessHost(host);
  if (!normalizedHost) return false;
  return normalizedAccess.hosts.includes(normalizedHost);
};
