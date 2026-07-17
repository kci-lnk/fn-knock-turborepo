import { computed } from "vue";
import { isAnySubdomainRoutingMode } from "@/lib/reverse-proxy-submode";
import { useConfigStore } from "@/store/config";
import type { HostMapping } from "@/types";

const normalizeDomainName = (value: string | null | undefined) =>
  String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/^\./, "")
    .replace(/\.$/, "");

const isHostWithinDomain = (host: string, domain: string): boolean => {
  const normalizedHost = normalizeDomainName(host);
  const normalizedDomain = normalizeDomainName(domain);
  if (!normalizedHost || !normalizedDomain) return false;
  return (
    normalizedHost === normalizedDomain ||
    normalizedHost.endsWith(`.${normalizedDomain}`)
  );
};

const isAuthServiceMapping = (mapping: HostMapping): boolean =>
  mapping.service_role === "auth";

export const useSessionCookieScope = () => {
  const configStore = useConfigStore();
  const isDirectMode = computed(() => configStore.config?.run_type === 0);
  const isSubdomainRoutingMode = computed(() =>
    isAnySubdomainRoutingMode(configStore.config),
  );
  const effectiveSharedCookieDomain = computed(() => {
    const explicit = configStore.config?.subdomain_mode?.cookie_domain?.trim();
    if (explicit) return explicit;
    return configStore.config?.subdomain_mode?.root_domain?.trim() || "";
  });
  const incompatibleCookieScopeHosts = computed(() => {
    if (!isSubdomainRoutingMode.value) return [];
    const sharedDomain = normalizeDomainName(effectiveSharedCookieDomain.value);
    return (configStore.config?.host_mappings ?? [])
      .filter((mapping) => mapping.use_auth && !isAuthServiceMapping(mapping))
      .map((mapping) => normalizeDomainName(mapping.host))
      .filter(
        (host): host is string =>
          Boolean(host) &&
          (!sharedDomain || !isHostWithinDomain(host, sharedDomain)),
      );
  });

  return {
    effectiveSharedCookieDomain,
    incompatibleCookieScopeHosts,
    isDirectMode,
    isSubdomainRoutingMode,
  };
};
