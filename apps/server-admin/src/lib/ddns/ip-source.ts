import { IPDetector } from "../../plugins/ip-detector";
import {
  findDDNSNetworkInterface,
  listDDNSNetworkInterfaces,
  listSelectableDDNSInterfaceAddresses,
  normalizeNetworkInterface,
} from "./network";
import {
  ddnsTranslate,
  getUpdateScopeDetectionOptions,
} from "./providers/helpers";
import type { DDNSIpSource, DDNSUpdateScope } from "./types";

export const DDNS_IP_SOURCE_FIELD = "ip_source";
export const DDNS_INTERFACE_IPV4_INDEX_FIELD = "interface_ipv4_index";
export const DDNS_INTERFACE_IPV6_INDEX_FIELD = "interface_ipv6_index";
export const DEFAULT_DDNS_IP_SOURCE: DDNSIpSource = "public";
export const DEFAULT_DDNS_INTERFACE_ADDRESS_INDEX = "";

const ddnsT = ddnsTranslate;

export type DDNSResolvedTargetIPs = {
  ipv4: string | null;
  ipv6: string | null;
  source: DDNSIpSource;
  sourceLabel: string;
  warnings: string[];
};

export function normalizeIpSource(
  value: string | null | undefined,
): DDNSIpSource {
  return value === "interface" ? "interface" : DEFAULT_DDNS_IP_SOURCE;
}

export function normalizeInterfaceAddressIndex(
  value: string | null | undefined,
): string {
  const trimmed = value?.trim() || "";
  if (!trimmed) {
    return DEFAULT_DDNS_INTERFACE_ADDRESS_INDEX;
  }

  const parsed = Number(trimmed);
  if (!Number.isInteger(parsed) || parsed < 0) {
    return DEFAULT_DDNS_INTERFACE_ADDRESS_INDEX;
  }

  return String(parsed);
}

export function getDDNSIpSourceLabel(
  source: DDNSIpSource,
  networkInterface?: string | null,
): string {
  if (source === "interface") {
    const normalizedInterface = normalizeNetworkInterface(networkInterface);
    return normalizedInterface
      ? ddnsT("interfaceSourceLabel", { name: normalizedInterface })
      : ddnsT("selectedInterfaceSourceLabel");
  }

  return ddnsT("publicSourceLabel");
}

export function getDDNSTargetIPUnavailableMessage(
  source: DDNSIpSource,
  scope: DDNSUpdateScope,
): string {
  if (source === "interface") {
    if (scope === "ipv6_only") {
      return ddnsT("interfaceIpv6Unavailable");
    }
    if (scope === "ipv4_only") {
      return ddnsT("interfaceIpv4Unavailable");
    }
    return ddnsT("interfaceDualStackUnavailable");
  }

  if (scope === "ipv6_only") {
    return ddnsT("publicIpv6Unavailable");
  }
  if (scope === "ipv4_only") {
    return ddnsT("publicIpv4Unavailable");
  }
  return ddnsT("publicDualStackUnavailable");
}

function resolveInterfaceAddress(
  interfaceName: string,
  family: "ipv4" | "ipv6",
  index: string | null | undefined,
): string | null {
  const candidates = listSelectableDDNSInterfaceAddresses(
    interfaceName,
    family,
  );
  if (candidates.length === 0) {
    return null;
  }

  const normalizedIndex = normalizeInterfaceAddressIndex(index);
  if (!normalizedIndex) {
    throw new Error(
      ddnsT("selectInterfaceAddress", {
        family: family === "ipv4" ? "IPv4" : "IPv6",
      }),
    );
  }

  const selected = candidates[Number(normalizedIndex)];
  if (!selected) {
    throw new Error(
      ddnsT("selectedInterfaceAddressUnavailable", {
        index: Number(normalizedIndex) + 1,
        family: family === "ipv4" ? "IPv4" : "IPv6",
      }),
    );
  }

  return selected.address;
}

function listKnownSelectableIPv6Addresses(interfaceName?: string): string[] {
  if (interfaceName) {
    return listSelectableDDNSInterfaceAddresses(interfaceName, "ipv6").map(
      (item) => item.address,
    );
  }

  return listDDNSNetworkInterfaces().flatMap((item) =>
    item.selectableAddresses
      .filter((address) => address.family === "ipv6")
      .map((address) => address.address),
  );
}

export async function resolveDDNSTargetIPs(options: {
  updateScope: DDNSUpdateScope;
  networkInterface?: string | null;
  ipSource?: string | null;
  interfaceIpv4Index?: string | null;
  interfaceIpv6Index?: string | null;
}): Promise<DDNSResolvedTargetIPs> {
  const source = normalizeIpSource(options.ipSource);
  const detectionOptions = getUpdateScopeDetectionOptions(options.updateScope);
  const normalizedInterface = normalizeNetworkInterface(
    options.networkInterface,
  );

  if (source === "public") {
    const ips = await IPDetector.getCurrentIPs({
      networkInterface: normalizedInterface,
      ...detectionOptions,
    });
    const warnings: string[] = [];

    if (detectionOptions.enableIPv4 && ips.errors.ipv4) {
      warnings.push(
        ips.ipv6
          ? ddnsT("ipv4FailedContinueIpv6", { error: ips.errors.ipv4 })
          : ddnsT("ipv4Failed", { error: ips.errors.ipv4 }),
      );
    }
    if (detectionOptions.enableIPv6 && ips.errors.ipv6) {
      warnings.push(
        ips.ipv4
          ? ddnsT("ipv6FailedContinueIpv4", { error: ips.errors.ipv6 })
          : ddnsT("ipv6Failed", { error: ips.errors.ipv6 }),
      );
    }
    if (detectionOptions.enableIPv6 && ips.ipv6) {
      const knownIPv6Addresses =
        listKnownSelectableIPv6Addresses(normalizedInterface);
      if (
        knownIPv6Addresses.length > 0 &&
        !knownIPv6Addresses.includes(ips.ipv6)
      ) {
        warnings.push(ddnsT("publicIpv6NotSelectable", { ip: ips.ipv6 }));
      }
    }

    return {
      ipv4: ips.ipv4,
      ipv6: ips.ipv6,
      source,
      sourceLabel: getDDNSIpSourceLabel(source, normalizedInterface),
      warnings,
    };
  }

  if (!normalizedInterface) {
    throw new Error(ddnsT("interfaceRequired"));
  }

  const selectedInterface = findDDNSNetworkInterface(normalizedInterface);
  if (!selectedInterface) {
    throw new Error(ddnsT("interfaceNotFound", { name: normalizedInterface }));
  }

  return {
    ipv4: detectionOptions.enableIPv4
      ? resolveInterfaceAddress(
          selectedInterface.name,
          "ipv4",
          options.interfaceIpv4Index,
        )
      : null,
    ipv6: detectionOptions.enableIPv6
      ? resolveInterfaceAddress(
          selectedInterface.name,
          "ipv6",
          options.interfaceIpv6Index,
        )
      : null,
    source,
    sourceLabel: getDDNSIpSourceLabel(source, selectedInterface.name),
    warnings: [],
  };
}
