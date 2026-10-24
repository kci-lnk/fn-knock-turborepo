import type { DDNSSettingsPayload } from "@/lib/api/ddns";

export type DDNSUpdateProxy = DDNSSettingsPayload["updateProxy"];
export type DDNSUpdateProxyDraft = DDNSUpdateProxy & {
  password: string;
  clearPassword: boolean;
};
export type DDNSUpdateProxyMode = "inherit" | "direct" | "proxy";

export const normalizeUpdateProxy = (
  value?: Partial<DDNSUpdateProxy> | null,
): DDNSUpdateProxy => ({
  enabled: value?.enabled === true,
  url: value?.url ?? "",
  username: value?.username ?? "",
  passwordConfigured: value?.passwordConfigured === true,
});
export const createUpdateProxyDraft = (
  value?: Partial<DDNSUpdateProxy> | null,
): DDNSUpdateProxyDraft => ({
  ...normalizeUpdateProxy(value),
  password: "",
  clearPassword: false,
});
export const normalizeUpdateProxyMode = (
  value?: string | null,
): DDNSUpdateProxyMode =>
  value === "direct" || value === "proxy" ? value : "inherit";

export const updateProxyValidationKey = (
  value: DDNSUpdateProxyDraft,
): string | null => {
  const address = value.url.trim();
  if (!address) return value.enabled ? "admin.ddns.updateProxyMissing" : null;
  try {
    const url = new URL(address);
    if (
      !["http:", "https:", "socks5:", "socks5h:"].includes(url.protocol) ||
      !url.hostname ||
      url.port === "0" ||
      (url.protocol.startsWith("socks") && !url.port) ||
      url.username ||
      url.password ||
      !["", "/"].includes(url.pathname) ||
      url.search ||
      url.hash ||
      [...address].some(
        (char) => char.charCodeAt(0) < 32 || char.charCodeAt(0) === 127,
      ) ||
      [...value.username].some((char) =>
        [":", "\r", "\n", "\0"].includes(char),
      ) ||
      [...value.password].some((char) => ["\r", "\n", "\0"].includes(char)) ||
      (!value.username &&
        (value.password ||
          (value.passwordConfigured && !value.clearPassword))) ||
      (url.protocol.startsWith("socks") &&
        (new TextEncoder().encode(value.username).length > 255 ||
          new TextEncoder().encode(value.password).length > 255))
    ) {
      return "admin.ddns.updateProxyInvalid";
    }
  } catch {
    return "admin.ddns.updateProxyInvalid";
  }
  return null;
};
