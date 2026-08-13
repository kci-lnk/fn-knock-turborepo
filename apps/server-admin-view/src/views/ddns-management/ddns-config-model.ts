import type {
  DDNSHttpTransport,
  DDNSInterfaceSelector,
  DDNSIpSource,
  DDNSNetworkInterfacePayload,
  DDNSPublicCheckSourcesPayload,
  DDNSPublicDnsProvider,
  DDNSTargetSummaryPayload,
  DDNSUpdateScope,
} from "@/lib/api/ddns";
import {
  ALLOW_PRIVATE_ADDRESSES_KEY,
  DEFAULT_DDNS_HTTP_TRANSPORT,
  DEFAULT_DDNS_IP_SOURCE,
  DEFAULT_DDNS_PUBLIC_DNS_PROVIDER,
  DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES,
  DEFAULT_DDNS_UPDATE_SCOPE,
  EMPTY_DDNS_PUBLIC_CHECK_SOURCES,
  INTERFACE_IPV4_INDEX_KEY,
  INTERFACE_IPV4_SELECTOR_KEY,
  INTERFACE_IPV6_INDEX_KEY,
  INTERFACE_IPV6_SELECTOR_KEY,
  IP_SOURCE_KEY,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
  NETWORK_INTERFACE_AUTO_VALUE,
  NETWORK_INTERFACE_KEY,
  SOURCE_DOMAIN_KEY,
  STATIC_IPV4_KEY,
  STATIC_IPV6_KEY,
  UPDATE_SCOPE_KEY,
  type Provider,
} from "./ddns-model-types";

export const normalizeUpdateScope = (
  value: string | null | undefined,
): DDNSUpdateScope => {
  if (
    value === "dual_stack" ||
    value === "ipv6_only" ||
    value === "ipv4_only"
  ) {
    return value;
  }
  return DEFAULT_DDNS_UPDATE_SCOPE;
};

export const normalizeIpSource = (
  value: string | null | undefined,
): DDNSIpSource => {
  if (value === "interface" || value === "static" || value === "domain") {
    return value;
  }
  return DEFAULT_DDNS_IP_SOURCE;
};

export const normalizeDDNSHttpTransport = (
  value: string | null | undefined,
): DDNSHttpTransport => {
  if (value === "node" || value === "fetch") {
    return "node";
  }
  if (value === "curl") {
    return "curl";
  }
  return DEFAULT_DDNS_HTTP_TRANSPORT;
};

export const normalizeDDNSPublicDnsProvider = (
  value: string | null | undefined,
): DDNSPublicDnsProvider => {
  if (
    value === "none" ||
    value === "alidns" ||
    value === "tencent" ||
    value === "cloudflare" ||
    value === "google"
  ) {
    return value;
  }
  return DEFAULT_DDNS_PUBLIC_DNS_PROVIDER;
};

export const normalizeNetworkInterface = (value: string | null | undefined) =>
  value?.trim() || "";

export const normalizeConfigBoolean = (
  value: string | null | undefined,
): "true" | "false" =>
  value?.trim().toLowerCase() === "true" ? "true" : "false";

export const allowsPrivateAddresses = (config: Record<string, string>) =>
  normalizeConfigBoolean(config[ALLOW_PRIVATE_ADDRESSES_KEY]) === "true";

export const normalizeInterfaceAddressIndex = (
  value: string | null | undefined,
) => {
  const trimmed = value?.trim() || "";
  if (!trimmed) {
    return "";
  }

  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 0) {
    return "";
  }

  return String(parsed);
};

export const createDefaultInterfaceSelector = (): DDNSInterfaceSelector => ({
  version: 1,
  mode: "auto",
  allowTemporary: false,
});

export const parseInterfaceSelector = (
  value: string | null | undefined,
): DDNSInterfaceSelector | null => {
  const raw = value?.trim();
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as Partial<DDNSInterfaceSelector>;
    if (
      parsed.version !== 1 ||
      (parsed.mode !== "auto" && parsed.mode !== "rules") ||
      (parsed.preferredAddress !== undefined &&
        typeof parsed.preferredAddress !== "string") ||
      (parsed.includeCidrs !== undefined &&
        !Array.isArray(parsed.includeCidrs)) ||
      (parsed.excludeCidrs !== undefined &&
        !Array.isArray(parsed.excludeCidrs)) ||
      (parsed.ipv6InterfaceId !== undefined &&
        typeof parsed.ipv6InterfaceId !== "string") ||
      typeof parsed.allowTemporary !== "boolean"
    ) {
      return null;
    }
    return {
      version: 1,
      mode: parsed.mode,
      ...(parsed.preferredAddress?.trim()
        ? { preferredAddress: parsed.preferredAddress.trim() }
        : {}),
      ...(parsed.includeCidrs?.length
        ? {
            includeCidrs: parsed.includeCidrs
              .map(String)
              .map((item) => item.trim())
              .filter(Boolean),
          }
        : {}),
      ...(parsed.excludeCidrs?.length
        ? {
            excludeCidrs: parsed.excludeCidrs
              .map(String)
              .map((item) => item.trim())
              .filter(Boolean),
          }
        : {}),
      ...(parsed.ipv6InterfaceId?.trim()
        ? { ipv6InterfaceId: parsed.ipv6InterfaceId.trim() }
        : {}),
      allowTemporary: parsed.allowTemporary,
    };
  } catch {
    return null;
  }
};

export const serializeInterfaceSelector = (selector: DDNSInterfaceSelector) =>
  JSON.stringify({
    version: 1,
    mode: selector.mode,
    ...(selector.preferredAddress?.trim()
      ? { preferredAddress: selector.preferredAddress.trim() }
      : {}),
    ...(selector.includeCidrs?.length
      ? { includeCidrs: selector.includeCidrs.map((item) => item.trim()) }
      : {}),
    ...(selector.excludeCidrs?.length
      ? { excludeCidrs: selector.excludeCidrs.map((item) => item.trim()) }
      : {}),
    ...(selector.ipv6InterfaceId?.trim()
      ? { ipv6InterfaceId: selector.ipv6InterfaceId.trim() }
      : {}),
    allowTemporary: selector.allowTemporary,
  } satisfies DDNSInterfaceSelector);

export const normalizeInterfaceSelectorConfig = (
  value: string | null | undefined,
) => {
  const selector = parseInterfaceSelector(value);
  return selector ? serializeInterfaceSelector(selector) : "";
};

const expandIPv6Address = (address: string): string[] | null => {
  const value = address.trim().toLowerCase();
  if (!value || value.includes(".")) return null;
  const halves = value.split("::");
  if (halves.length > 2) return null;
  const left = halves[0] ? halves[0].split(":") : [];
  const right = halves.length === 2 && halves[1] ? halves[1].split(":") : [];
  if ([...left, ...right].some((part) => !/^[0-9a-f]{1,4}$/.test(part))) {
    return null;
  }
  const missing = 8 - left.length - right.length;
  if ((halves.length === 1 && missing !== 0) || missing < 0) return null;
  return [...left, ...Array(missing).fill("0"), ...right].map((part) =>
    part.padStart(4, "0"),
  );
};

export const ipv6InterfaceIdFromAddress = (address: string) => {
  const expanded = expandIPv6Address(address);
  return expanded ? expanded.slice(4).join(":") : "";
};

export const buildInterfaceSelectorFromLegacyIndex = (
  option: DDNSNetworkInterfacePayload | null | undefined,
  family: "ipv4" | "ipv6",
  legacyIndex: string | null | undefined,
  currentAddress?: string | null,
  allowPrivateAddresses = false,
): { selector: DDNSInterfaceSelector; migrated: boolean } => {
  const selector = createDefaultInterfaceSelector();
  const legacyIndexValue = legacyIndex?.trim();
  if (!legacyIndexValue) {
    return { selector, migrated: false };
  }
  const index = Number(legacyIndexValue);
  const candidates = buildInterfaceAddressCandidates(
    option,
    allowPrivateAddresses,
  ).filter((item) => item.family === family);
  const usable = candidates.filter(
    (item) => !item.tentative && !item.dadFailed && !item.deprecated,
  );
  const current = currentAddress?.trim().toLowerCase();
  let candidate = current
    ? usable.find((item) => item.address.toLowerCase() === current)
    : undefined;
  if (!candidate && family === "ipv6" && current) {
    const interfaceId = ipv6InterfaceIdFromAddress(current);
    if (interfaceId) {
      candidate = usable
        .filter(
          (item) => ipv6InterfaceIdFromAddress(item.address) === interfaceId,
        )
        .sort((left, right) => left.address.localeCompare(right.address))[0];
    }
  }
  if (!candidate && Number.isInteger(index) && index >= 0) {
    const indexedCandidate = candidates[index];
    if (
      indexedCandidate &&
      !indexedCandidate.tentative &&
      !indexedCandidate.dadFailed &&
      !indexedCandidate.deprecated
    ) {
      candidate = indexedCandidate;
    }
  }
  if (!candidate) {
    return { selector, migrated: false };
  }
  selector.preferredAddress = candidate.address;
  selector.allowTemporary = candidate.temporary === true;
  return { selector, migrated: true };
};

export const normalizeStaticIPAddress = (value: string | null | undefined) =>
  value?.trim() || "";

export const normalizeSourceDomain = (value: string | null | undefined) =>
  (value || "").trim().replace(/\.+$/, "");

export const normalizeUpdateIntervalMinutes = (value: unknown) => {
  const parsed = Number(value);
  if (
    Number.isInteger(parsed) &&
    parsed >= MIN_DDNS_UPDATE_INTERVAL_MINUTES &&
    parsed <= MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return parsed;
  }

  return DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES;
};

export const normalizePublicCheckSources = (
  value: Partial<DDNSPublicCheckSourcesPayload> | null | undefined,
  fallback: DDNSPublicCheckSourcesPayload = EMPTY_DDNS_PUBLIC_CHECK_SOURCES,
): DDNSPublicCheckSourcesPayload => ({
  ipv4: Array.isArray(value?.ipv4)
    ? value.ipv4.map((item) => String(item ?? ""))
    : [...fallback.ipv4],
  ipv6: Array.isArray(value?.ipv6)
    ? value.ipv6.map((item) => String(item ?? ""))
    : [...fallback.ipv6],
});

export const toNetworkInterfaceSelectValue = (
  value: string | null | undefined,
) => normalizeNetworkInterface(value) || NETWORK_INTERFACE_AUTO_VALUE;

export const normalizeTargetConfigValues = (
  config: Record<string, string> | null | undefined,
): Record<string, string> => ({
  ...(config || {}),
  [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config?.[UPDATE_SCOPE_KEY]),
  [IP_SOURCE_KEY]: normalizeIpSource(config?.[IP_SOURCE_KEY]),
  [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
    config?.[NETWORK_INTERFACE_KEY],
  ),
  [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config?.[INTERFACE_IPV4_INDEX_KEY],
  ),
  [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config?.[INTERFACE_IPV6_INDEX_KEY],
  ),
  [INTERFACE_IPV4_SELECTOR_KEY]: normalizeInterfaceSelectorConfig(
    config?.[INTERFACE_IPV4_SELECTOR_KEY],
  ),
  [INTERFACE_IPV6_SELECTOR_KEY]: normalizeInterfaceSelectorConfig(
    config?.[INTERFACE_IPV6_SELECTOR_KEY],
  ),
  [ALLOW_PRIVATE_ADDRESSES_KEY]: normalizeConfigBoolean(
    config?.[ALLOW_PRIVATE_ADDRESSES_KEY],
  ),
  [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config?.[STATIC_IPV4_KEY]),
  [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config?.[STATIC_IPV6_KEY]),
  [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config?.[SOURCE_DOMAIN_KEY]),
});

export const extractCommonTargetConfig = (
  config: Record<string, string>,
): Record<string, string> => ({
  [UPDATE_SCOPE_KEY]: normalizeUpdateScope(config[UPDATE_SCOPE_KEY]),
  [IP_SOURCE_KEY]: normalizeIpSource(config[IP_SOURCE_KEY]),
  [NETWORK_INTERFACE_KEY]: normalizeNetworkInterface(
    config[NETWORK_INTERFACE_KEY],
  ),
  [INTERFACE_IPV4_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV4_INDEX_KEY],
  ),
  [INTERFACE_IPV6_INDEX_KEY]: normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV6_INDEX_KEY],
  ),
  [INTERFACE_IPV4_SELECTOR_KEY]: normalizeInterfaceSelectorConfig(
    config[INTERFACE_IPV4_SELECTOR_KEY],
  ),
  [INTERFACE_IPV6_SELECTOR_KEY]: normalizeInterfaceSelectorConfig(
    config[INTERFACE_IPV6_SELECTOR_KEY],
  ),
  [ALLOW_PRIVATE_ADDRESSES_KEY]: normalizeConfigBoolean(
    config[ALLOW_PRIVATE_ADDRESSES_KEY],
  ),
  [STATIC_IPV4_KEY]: normalizeStaticIPAddress(config[STATIC_IPV4_KEY]),
  [STATIC_IPV6_KEY]: normalizeStaticIPAddress(config[STATIC_IPV6_KEY]),
  [SOURCE_DOMAIN_KEY]: normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]),
});

export const resolveNetworkInterfaceOptions = (
  items: DDNSNetworkInterfacePayload[],
  selected: string,
  unavailable: { label: string; summary: string },
) => {
  const resolved = [...items];
  if (selected && !resolved.some((item) => item.name === selected)) {
    resolved.push({
      name: selected,
      label: unavailable.label,
      summary: unavailable.summary,
      hasIpv4: false,
      hasIpv6: false,
      addresses: [],
      selectableAddresses: [],
      privateAddresses: [],
    });
  }
  return resolved;
};

export const findProviderDef = (
  providers: Provider[],
  providerName: string,
): Provider | null =>
  providers.find((provider) => provider.name === providerName) || null;

export const hasConfiguredProviderFields = (
  provider: Provider | null,
  config: Record<string, string>,
) =>
  provider?.fields.some(
    (field) => String(config[field.key] ?? "").trim() !== "",
  ) === true;

export const normalizeProviderConfigForComparison = (
  config: Record<string, string>,
) => {
  const normalized = normalizeTargetConfigValues(config);
  return Object.keys(normalized)
    .sort()
    .reduce<Record<string, string>>((result, key) => {
      result[key] = String(normalized[key] ?? "");
      return result;
    }, {});
};

export const isProviderConfigEqual = (
  left: Record<string, string>,
  right: Record<string, string>,
) =>
  JSON.stringify(normalizeProviderConfigForComparison(left)) ===
  JSON.stringify(normalizeProviderConfigForComparison(right));

export const isSingleAddressProvider = (
  providers: Provider[],
  providerName: string,
) =>
  findProviderDef(providers, providerName)?.capabilities?.addressMode ===
  "single_address";

export const isUpdateScopeOptionDisabled = (
  providers: Provider[],
  providerName: string,
  option: DDNSUpdateScope,
) =>
  isSingleAddressProvider(providers, providerName) && option === "dual_stack";

export const isIpSourceOptionDisabled = (
  providers: Provider[],
  providerName: string,
  option: DDNSIpSource,
) => {
  const supportedSources = findProviderDef(providers, providerName)
    ?.capabilities?.ipSources;
  return Array.isArray(supportedSources) && !supportedSources.includes(option);
};

export const shouldShowIPv4ForScope = (value: string | null | undefined) =>
  normalizeUpdateScope(value) !== "ipv6_only";

export const shouldShowIPv6ForScope = (value: string | null | undefined) =>
  normalizeUpdateScope(value) !== "ipv4_only";

export const getTargetDisplayName = (target: DDNSTargetSummaryPayload) =>
  target.name || target.domainSummary || target.providerLabel;

export const buildNetworkInterfaceAddressOptions = (
  option: DDNSNetworkInterfacePayload | null | undefined,
  family: "ipv4" | "ipv6",
  getLabel: (
    item: { address: string; family: "ipv4" | "ipv6" },
    index: number,
  ) => string,
  allowPrivateAddresses = false,
) =>
  buildInterfaceAddressCandidates(option, allowPrivateAddresses)
    .filter((item) => item.family === family)
    .map((item, index) => ({
      value: String(index),
      label: getLabel(item, index),
    }));

export const buildInterfaceAddressCandidates = (
  option: DDNSNetworkInterfacePayload | null | undefined,
  allowPrivateAddresses: boolean,
) => {
  const candidates = [...(option?.selectableAddresses || [])];
  if (!allowPrivateAddresses) return candidates;
  for (const candidate of option?.privateAddresses || []) {
    if (
      !candidates.some(
        (existing) =>
          existing.family === candidate.family &&
          existing.address === candidate.address,
      )
    ) {
      candidates.push(candidate);
    }
  }
  return candidates;
};
