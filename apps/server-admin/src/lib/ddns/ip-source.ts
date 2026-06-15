import { lookup } from "node:dns/promises";
import { isIP } from "node:net";
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
  normalizeDomain,
} from "./providers/helpers";
import type { DDNSIpSource, DDNSUpdateScope } from "./types";

export const DDNS_IP_SOURCE_FIELD = "ip_source";
export const DDNS_INTERFACE_IPV4_INDEX_FIELD = "interface_ipv4_index";
export const DDNS_INTERFACE_IPV6_INDEX_FIELD = "interface_ipv6_index";
export const DDNS_STATIC_IPV4_FIELD = "static_ipv4";
export const DDNS_STATIC_IPV6_FIELD = "static_ipv6";
export const DDNS_SOURCE_DOMAIN_FIELD = "source_domain";
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
  if (value === "interface" || value === "static" || value === "domain") {
    return value;
  }
  return DEFAULT_DDNS_IP_SOURCE;
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
  sourceDomain?: string | null,
): string {
  if (source === "interface") {
    const normalizedInterface = normalizeNetworkInterface(networkInterface);
    return normalizedInterface
      ? ddnsT("interfaceSourceLabel", { name: normalizedInterface })
      : ddnsT("selectedInterfaceSourceLabel");
  }

  if (source === "static") {
    return ddnsT("staticSourceLabel");
  }

  if (source === "domain") {
    const domain = normalizeSourceDomain(sourceDomain);
    return domain
      ? ddnsT("domainSourceLabel", { domain })
      : ddnsT("domainSourceLabelEmpty");
  }

  return ddnsT("publicSourceLabel");
}

export function getDDNSTargetIPUnavailableMessage(
  source: DDNSIpSource,
  scope: DDNSUpdateScope,
): string {
  if (source === "static") {
    if (scope === "ipv6_only") {
      return ddnsT("staticIpv6Unavailable");
    }
    if (scope === "ipv4_only") {
      return ddnsT("staticIpv4Unavailable");
    }
    return ddnsT("staticDualStackUnavailable");
  }

  if (source === "domain") {
    if (scope === "ipv6_only") {
      return ddnsT("domainIpv6Unavailable");
    }
    if (scope === "ipv4_only") {
      return ddnsT("domainIpv4Unavailable");
    }
    return ddnsT("domainDualStackUnavailable");
  }

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

export function normalizeStaticIPAddress(
  value: string | null | undefined,
): string {
  return value?.trim() || "";
}

export function normalizeSourceDomain(
  value: string | null | undefined,
): string {
  return normalizeDomain(value || "");
}

function isValidSourceDomain(value: string): boolean {
  const domain = normalizeSourceDomain(value);
  if (!domain || domain.length > 253) {
    return false;
  }
  if (
    /^https?:\/\//i.test(value) ||
    domain.includes("/") ||
    domain.includes(":") ||
    domain.includes("*") ||
    /\s/.test(domain)
  ) {
    return false;
  }

  return domain.split(".").every((label) => {
    return (
      label.length > 0 &&
      label.length <= 63 &&
      !label.startsWith("-") &&
      !label.endsWith("-") &&
      /^[a-z0-9-]+$/i.test(label)
    );
  });
}

function resolveStaticAddress(
  value: string | null | undefined,
  family: "ipv4" | "ipv6",
): string | null {
  const address = normalizeStaticIPAddress(value);
  if (!address) {
    return null;
  }

  const expectedVersion = family === "ipv4" ? 4 : 6;
  if (isIP(address) !== expectedVersion) {
    throw new Error(
      family === "ipv4"
        ? ddnsT("staticIpv4Invalid", { value: address })
        : ddnsT("staticIpv6Invalid", { value: address }),
    );
  }

  return address;
}

async function resolveSourceDomainAddresses(
  rawDomain: string | null | undefined,
): Promise<{ ipv4: string | null; ipv6: string | null }> {
  const domain = normalizeSourceDomain(rawDomain);
  if (!domain) {
    throw new Error(ddnsT("sourceDomainRequired"));
  }
  if (!isValidSourceDomain(domain)) {
    throw new Error(ddnsT("sourceDomainInvalid", { domain }));
  }

  try {
    const entries = await lookup(domain, { all: true, verbatim: false });
    const ipv4 =
      entries.find((entry) => entry.family === 4)?.address?.trim() || null;
    const ipv6 =
      entries.find((entry) => entry.family === 6)?.address?.trim() || null;
    return { ipv4, ipv6 };
  } catch (error) {
    throw new Error(
      ddnsT("sourceDomainResolveFailed", {
        domain,
        error: error instanceof Error ? error.message : String(error),
      }),
    );
  }
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
  staticIpv4?: string | null;
  staticIpv6?: string | null;
  sourceDomain?: string | null;
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

  if (source === "static") {
    return {
      ipv4: detectionOptions.enableIPv4
        ? resolveStaticAddress(options.staticIpv4, "ipv4")
        : null,
      ipv6: detectionOptions.enableIPv6
        ? resolveStaticAddress(options.staticIpv6, "ipv6")
        : null,
      source,
      sourceLabel: getDDNSIpSourceLabel(source),
      warnings: [],
    };
  }

  if (source === "domain") {
    const resolved = await resolveSourceDomainAddresses(options.sourceDomain);
    const domain = normalizeSourceDomain(options.sourceDomain);
    return {
      ipv4: detectionOptions.enableIPv4 ? resolved.ipv4 : null,
      ipv6: detectionOptions.enableIPv6 ? resolved.ipv6 : null,
      source,
      sourceLabel: getDDNSIpSourceLabel(source, null, domain),
      warnings: [],
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
