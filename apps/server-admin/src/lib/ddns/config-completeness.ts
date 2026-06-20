import { isIP } from "node:net";
import {
  DDNS_INTERFACE_IPV4_INDEX_FIELD,
  DDNS_INTERFACE_IPV6_INDEX_FIELD,
  DDNS_IP_SOURCE_FIELD,
  DDNS_SOURCE_DOMAIN_FIELD,
  DDNS_STATIC_IPV4_FIELD,
  DDNS_STATIC_IPV6_FIELD,
  normalizeInterfaceAddressIndex,
  normalizeIpSource,
  normalizeSourceDomain,
  normalizeStaticIPAddress,
} from "./ip-source";
import {
  DDNS_NETWORK_INTERFACE_FIELD,
  findDDNSNetworkInterface,
  normalizeNetworkInterface,
} from "./network";
import {
  DDNS_UPDATE_SCOPE_FIELD,
  normalizeUpdateScope,
} from "./providers/helpers";
import type { DDNSProviderDefinition, DDNSTargetRecord } from "./types";

export const isDDNSTargetConfigComplete = (
  target: DDNSTargetRecord,
  definition: DDNSProviderDefinition | null,
): boolean => {
  if (!definition) {
    return false;
  }

  const requiredFields = definition.fields.filter(
    (field) => field.required !== false,
  );
  const providerFieldsComplete = requiredFields.every(
    (field) => !!target.config[field.key],
  );
  if (!providerFieldsComplete) {
    return false;
  }

  const updateScope = normalizeUpdateScope(
    target.config[DDNS_UPDATE_SCOPE_FIELD],
  );
  if (
    definition.capabilities?.addressMode === "single_address" &&
    updateScope === "dual_stack"
  ) {
    return false;
  }

  const ipSource = normalizeIpSource(target.config[DDNS_IP_SOURCE_FIELD]);
  if (ipSource === "static") {
    const requiresIPv4 = updateScope !== "ipv6_only";
    const requiresIPv6 = updateScope !== "ipv4_only";
    const ipv4 = normalizeStaticIPAddress(
      target.config[DDNS_STATIC_IPV4_FIELD],
    );
    const ipv6 = normalizeStaticIPAddress(
      target.config[DDNS_STATIC_IPV6_FIELD],
    );
    const hasValidIPv4 = isIP(ipv4) === 4;
    const hasValidIPv6 = isIP(ipv6) === 6;

    if (ipv4 && !hasValidIPv4) {
      return false;
    }
    if (ipv6 && !hasValidIPv6) {
      return false;
    }
    if (updateScope === "ipv4_only") {
      return hasValidIPv4;
    }
    if (updateScope === "ipv6_only") {
      return hasValidIPv6;
    }
    return (requiresIPv4 && hasValidIPv4) || (requiresIPv6 && hasValidIPv6);
  }

  if (ipSource === "domain") {
    return Boolean(
      normalizeSourceDomain(target.config[DDNS_SOURCE_DOMAIN_FIELD]),
    );
  }

  if (ipSource !== "interface") {
    return true;
  }

  const networkInterface = normalizeNetworkInterface(
    target.config[DDNS_NETWORK_INTERFACE_FIELD],
  );
  if (!networkInterface) {
    return false;
  }

  const network = findDDNSNetworkInterface(networkInterface);
  if (!network) {
    return false;
  }

  const requiresIPv4 = updateScope !== "ipv6_only";
  const requiresIPv6 = updateScope !== "ipv4_only";
  const hasSelectableIPv4 = network.selectableAddresses.some(
    (item) => item.family === "ipv4",
  );
  const hasSelectableIPv6 = network.selectableAddresses.some(
    (item) => item.family === "ipv6",
  );

  if (
    requiresIPv4 &&
    hasSelectableIPv4 &&
    !normalizeInterfaceAddressIndex(
      target.config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
    )
  ) {
    return false;
  }

  if (
    requiresIPv6 &&
    hasSelectableIPv6 &&
    !normalizeInterfaceAddressIndex(
      target.config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
    )
  ) {
    return false;
  }

  return true;
};
