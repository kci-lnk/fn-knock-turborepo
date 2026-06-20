import {
  EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
  isEdgeOneDDNSProvider,
  normalizeEdgeOneOverseasAccessMode,
} from "./providers/edgeone-shared";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  DDNS_SOURCE_DOMAIN_FIELD,
  DDNS_STATIC_IPV4_FIELD,
  DDNS_STATIC_IPV6_FIELD,
  DEFAULT_DDNS_IP_SOURCE,
  normalizeInterfaceAddressIndex,
  normalizeIpSource,
  normalizeSourceDomain,
  normalizeStaticIPAddress,
} from "./ip-source";
import {
  DDNS_NETWORK_INTERFACE_FIELD,
  normalizeNetworkInterface,
} from "./network";
import {
  DDNS_UPDATE_SCOPE_FIELD,
  normalizeUpdateScope,
} from "./providers/helpers";

export const normalizeDDNSConfig = (
  providerName: string | null | undefined,
  config: Record<string, string> | null | undefined,
): Record<string, string> => {
  const data = config || {};
  return {
    ...data,
    [DDNS_UPDATE_SCOPE_FIELD]: normalizeUpdateScope(
      data[DDNS_UPDATE_SCOPE_FIELD],
    ),
    [DDNS_IP_SOURCE_FIELD]: normalizeIpSource(data[DDNS_IP_SOURCE_FIELD]),
    [DDNS_NETWORK_INTERFACE_FIELD]: normalizeNetworkInterface(
      data[DDNS_NETWORK_INTERFACE_FIELD],
    ),
    [DDNS_INTERFACE_IPV4_INDEX_FIELD]: normalizeInterfaceAddressIndex(
      data[DDNS_INTERFACE_IPV4_INDEX_FIELD],
    ),
    [DDNS_INTERFACE_IPV6_INDEX_FIELD]: normalizeInterfaceAddressIndex(
      data[DDNS_INTERFACE_IPV6_INDEX_FIELD],
    ),
    [DDNS_STATIC_IPV4_FIELD]: normalizeStaticIPAddress(
      data[DDNS_STATIC_IPV4_FIELD],
    ),
    [DDNS_STATIC_IPV6_FIELD]: normalizeStaticIPAddress(
      data[DDNS_STATIC_IPV6_FIELD],
    ),
    [DDNS_SOURCE_DOMAIN_FIELD]: normalizeSourceDomain(
      data[DDNS_SOURCE_DOMAIN_FIELD],
    ),
    ...(isEdgeOneDDNSProvider(providerName || "")
      ? {
          [EDGEONE_OVERSEAS_ACCESS_MODE_FIELD]:
            normalizeEdgeOneOverseasAccessMode(
              data[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD],
            ),
        }
      : {}),
  };
};

export const prepareDDNSConfigForStorage = (
  providerName: string | null | undefined,
  config: Record<string, string>,
): Partial<Record<string, string>> => {
  const normalizedProviderName = providerName?.trim() || "";
  const ipSource = normalizeIpSource(config[DDNS_IP_SOURCE_FIELD]);
  const normalizedConfig: Partial<Record<string, string>> = {
    ...config,
    [DDNS_UPDATE_SCOPE_FIELD]: normalizeUpdateScope(
      config[DDNS_UPDATE_SCOPE_FIELD],
    ),
    [DDNS_IP_SOURCE_FIELD]: ipSource,
    [DDNS_NETWORK_INTERFACE_FIELD]: normalizeNetworkInterface(
      config[DDNS_NETWORK_INTERFACE_FIELD],
    ),
    [DDNS_INTERFACE_IPV4_INDEX_FIELD]: normalizeInterfaceAddressIndex(
      config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
    ),
    [DDNS_INTERFACE_IPV6_INDEX_FIELD]: normalizeInterfaceAddressIndex(
      config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
    ),
    [DDNS_STATIC_IPV4_FIELD]: normalizeStaticIPAddress(
      config[DDNS_STATIC_IPV4_FIELD],
    ),
    [DDNS_STATIC_IPV6_FIELD]: normalizeStaticIPAddress(
      config[DDNS_STATIC_IPV6_FIELD],
    ),
    [DDNS_SOURCE_DOMAIN_FIELD]: normalizeSourceDomain(
      config[DDNS_SOURCE_DOMAIN_FIELD],
    ),
    ...(isEdgeOneDDNSProvider(normalizedProviderName)
      ? {
          [EDGEONE_OVERSEAS_ACCESS_MODE_FIELD]:
            normalizeEdgeOneOverseasAccessMode(
              config[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD],
            ),
        }
      : {}),
  };

  if (ipSource === DEFAULT_DDNS_IP_SOURCE) {
    delete normalizedConfig[DDNS_IP_SOURCE_FIELD];
  }

  if (ipSource !== "interface") {
    delete normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD];
    delete normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD];
  } else {
    if (!normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD]) {
      delete normalizedConfig[DDNS_INTERFACE_IPV4_INDEX_FIELD];
    }
    if (!normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD]) {
      delete normalizedConfig[DDNS_INTERFACE_IPV6_INDEX_FIELD];
    }
  }

  if (ipSource !== "static") {
    delete normalizedConfig[DDNS_STATIC_IPV4_FIELD];
    delete normalizedConfig[DDNS_STATIC_IPV6_FIELD];
  } else {
    if (!normalizedConfig[DDNS_STATIC_IPV4_FIELD]) {
      delete normalizedConfig[DDNS_STATIC_IPV4_FIELD];
    }
    if (!normalizedConfig[DDNS_STATIC_IPV6_FIELD]) {
      delete normalizedConfig[DDNS_STATIC_IPV6_FIELD];
    }
  }

  if (ipSource !== "domain") {
    delete normalizedConfig[DDNS_SOURCE_DOMAIN_FIELD];
  } else if (!normalizedConfig[DDNS_SOURCE_DOMAIN_FIELD]) {
    delete normalizedConfig[DDNS_SOURCE_DOMAIN_FIELD];
  }

  if (
    !isEdgeOneDDNSProvider(normalizedProviderName) ||
    normalizedConfig[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD] === "off"
  ) {
    delete normalizedConfig[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD];
  }

  return normalizedConfig;
};

export const buildComparableDDNSConfigKey = (
  providerName: string | null | undefined,
  config: Record<string, string> | null | undefined,
): string => {
  const normalizedProviderName = providerName?.trim() || "";
  const prepared = prepareDDNSConfigForStorage(
    normalizedProviderName,
    normalizeDDNSConfig(normalizedProviderName, config || {}),
  );

  return JSON.stringify(
    Object.entries(prepared).sort(([left], [right]) =>
      left.localeCompare(right),
    ),
  );
};
