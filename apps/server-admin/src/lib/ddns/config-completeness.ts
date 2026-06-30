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
  ddnsProviderT,
  ddnsTranslate,
  normalizeUpdateScope,
} from "./providers/helpers";
import type { DDNSProviderDefinition, DDNSTargetRecord } from "./types";

export type DDNSTargetConfigCompleteness =
  | { complete: true; reason: null }
  | { complete: false; reason: string };

const ddnsT = ddnsTranslate;

const complete = (): DDNSTargetConfigCompleteness => ({
  complete: true,
  reason: null,
});

const incomplete = (reason: string): DDNSTargetConfigCompleteness => ({
  complete: false,
  reason,
});

const formatFamily = (family: "ipv4" | "ipv6") =>
  family === "ipv4" ? "IPv4" : "IPv6";

export const getDDNSTargetConfigCompleteness = (
  target: DDNSTargetRecord,
  definition: DDNSProviderDefinition | null,
): DDNSTargetConfigCompleteness => {
  if (!definition) {
    return incomplete(ddnsT("notConfigured"));
  }

  const requiredFields = definition.fields.filter(
    (field) => field.required !== false,
  );
  const missingRequiredFields = requiredFields.filter(
    (field) => !target.config[field.key],
  );
  if (missingRequiredFields.length > 0) {
    const missingLabels = missingRequiredFields
      .map((field) => field.label || field.key)
      .join(", ");
    return incomplete(
      `${ddnsProviderT(definition.name, "configIncomplete")}: ${missingLabels}`,
    );
  }

  const updateScope = normalizeUpdateScope(
    target.config[DDNS_UPDATE_SCOPE_FIELD],
  );
  if (
    definition.capabilities?.addressMode === "single_address" &&
    updateScope === "dual_stack"
  ) {
    return incomplete(
      ddnsT("singleAddressProviderUnsupported", {
        provider: definition.label,
      }),
    );
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
      return incomplete(ddnsT("staticIpv4Invalid", { value: ipv4 }));
    }
    if (ipv6 && !hasValidIPv6) {
      return incomplete(ddnsT("staticIpv6Invalid", { value: ipv6 }));
    }
    if (updateScope === "ipv4_only") {
      return hasValidIPv4
        ? complete()
        : incomplete(ddnsT("staticIpv4Unavailable"));
    }
    if (updateScope === "ipv6_only") {
      return hasValidIPv6
        ? complete()
        : incomplete(ddnsT("staticIpv6Unavailable"));
    }
    return (requiresIPv4 && hasValidIPv4) || (requiresIPv6 && hasValidIPv6)
      ? complete()
      : incomplete(ddnsT("staticDualStackUnavailable"));
  }

  if (ipSource === "domain") {
    return normalizeSourceDomain(target.config[DDNS_SOURCE_DOMAIN_FIELD])
      ? complete()
      : incomplete(ddnsT("sourceDomainRequired"));
  }

  if (ipSource !== "interface") {
    return complete();
  }

  const networkInterface = normalizeNetworkInterface(
    target.config[DDNS_NETWORK_INTERFACE_FIELD],
  );
  if (!networkInterface) {
    return incomplete(ddnsT("interfaceRequired"));
  }

  const network = findDDNSNetworkInterface(networkInterface);
  if (!network) {
    return incomplete(ddnsT("interfaceNotFound", { name: networkInterface }));
  }

  const requiresIPv4 = updateScope !== "ipv6_only";
  const requiresIPv6 = updateScope !== "ipv4_only";
  const selectableIPv4 = network.selectableAddresses.filter(
    (item) => item.family === "ipv4",
  );
  const selectableIPv6 = network.selectableAddresses.filter(
    (item) => item.family === "ipv6",
  );

  if (requiresIPv4 && selectableIPv4.length > 0) {
    const index = normalizeInterfaceAddressIndex(
      target.config[DDNS_INTERFACE_IPV4_INDEX_FIELD],
    );
    if (!index) {
      return incomplete(
        ddnsT("selectInterfaceAddress", { family: formatFamily("ipv4") }),
      );
    }
    if (!selectableIPv4[Number(index)]) {
      return incomplete(
        ddnsT("selectedInterfaceAddressUnavailable", {
          index: Number(index) + 1,
          family: formatFamily("ipv4"),
        }),
      );
    }
  }

  if (requiresIPv6 && selectableIPv6.length > 0) {
    const index = normalizeInterfaceAddressIndex(
      target.config[DDNS_INTERFACE_IPV6_INDEX_FIELD],
    );
    if (!index) {
      return incomplete(
        ddnsT("selectInterfaceAddress", { family: formatFamily("ipv6") }),
      );
    }
    if (!selectableIPv6[Number(index)]) {
      return incomplete(
        ddnsT("selectedInterfaceAddressUnavailable", {
          index: Number(index) + 1,
          family: formatFamily("ipv6"),
        }),
      );
    }
  }

  return complete();
};

export const isDDNSTargetConfigComplete = (
  target: DDNSTargetRecord,
  definition: DDNSProviderDefinition | null,
): boolean => getDDNSTargetConfigCompleteness(target, definition).complete;
