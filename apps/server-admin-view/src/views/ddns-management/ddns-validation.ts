import type { DDNSIpSource, DDNSUpdateScope } from "@/lib/api/ddns";
import {
  validateDDNSDomainTargets,
  type DDNSDomainTargetErrorCode,
} from "@/lib/ddns-domain";
import {
  INTERFACE_IPV4_SELECTOR_KEY,
  INTERFACE_IPV6_SELECTOR_KEY,
  IP_SOURCE_KEY,
  MAX_DDNS_UPDATE_INTERVAL_MINUTES,
  MIN_DDNS_UPDATE_INTERVAL_MINUTES,
  NETWORK_INTERFACE_KEY,
  SOURCE_DOMAIN_KEY,
  STATIC_IPV4_KEY,
  STATIC_IPV6_KEY,
  type Provider,
} from "./ddns-model-types";
import {
  findProviderDef,
  isSingleAddressProvider,
  normalizeIpSource,
  normalizeNetworkInterface,
  normalizeSourceDomain,
  normalizeStaticIPAddress,
  parseInterfaceSelector,
} from "./ddns-config-model";

export const isLikelyIPv4Address = (value: string) => {
  const parts = value.trim().split(".");
  return (
    parts.length === 4 &&
    parts.every((part) => {
      if (!/^\d{1,3}$/.test(part)) return false;
      const parsed = Number(part);
      return Number.isInteger(parsed) && parsed >= 0 && parsed <= 255;
    })
  );
};

export const isLikelyIPv6Address = (value: string) => {
  const address = value.trim();
  return (
    address.includes(":") &&
    address.length <= 45 &&
    /^[0-9a-f:.]+$/i.test(address)
  );
};

export const isLikelySourceDomain = (value: string) => {
  const domain = normalizeSourceDomain(value);
  if (!domain || domain.length > 253) return false;
  if (/^https?:\/\//i.test(value) || /[\s/:*]/.test(domain)) {
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
};

export type DDNSValidationParams = Record<string, string | number>;

export interface DDNSValidationIssue {
  messageKey: string;
  messageParams?: DDNSValidationParams;
  descriptionKey?: string;
  descriptionParams?: DDNSValidationParams;
}

export type DDNSAddressOption = { value: string; label: string };

const createValidationIssue = (
  messageKey: string,
  options: {
    messageParams?: DDNSValidationParams;
    descriptionKey?: string;
    descriptionParams?: DDNSValidationParams;
  } = {},
): DDNSValidationIssue => ({
  messageKey,
  ...options,
});

export const validateStaticIpSourceConfig = (
  config: Record<string, string>,
  updateScope: DDNSUpdateScope,
): DDNSValidationIssue | null => {
  const ipv4 = normalizeStaticIPAddress(config[STATIC_IPV4_KEY]);
  const ipv6 = normalizeStaticIPAddress(config[STATIC_IPV6_KEY]);
  const needsIPv4 = updateScope !== "ipv6_only";
  const needsIPv6 = updateScope !== "ipv4_only";

  if (needsIPv4 && ipv4 && !isLikelyIPv4Address(ipv4)) {
    return createValidationIssue("admin.ddns.invalidStaticIpv4");
  }

  if (needsIPv6 && ipv6 && !isLikelyIPv6Address(ipv6)) {
    return createValidationIssue("admin.ddns.invalidStaticIpv6");
  }

  if (updateScope === "ipv4_only" && !ipv4) {
    return createValidationIssue("admin.ddns.enterStaticIpv4");
  }

  if (updateScope === "ipv6_only" && !ipv6) {
    return createValidationIssue("admin.ddns.enterStaticIpv6");
  }

  if (updateScope === "dual_stack" && !ipv4 && !ipv6) {
    return createValidationIssue("admin.ddns.enterStaticIp");
  }

  return null;
};

export const validateDomainIpSourceConfig = (
  config: Record<string, string>,
): DDNSValidationIssue | null => {
  const domain = normalizeSourceDomain(config[SOURCE_DOMAIN_KEY]);

  if (!domain) {
    return createValidationIssue("admin.ddns.enterSourceDomain");
  }

  if (!isLikelySourceDomain(domain)) {
    return createValidationIssue("admin.ddns.invalidSourceDomain");
  }

  return null;
};

const validateProviderConfigFields = (
  config: Record<string, string>,
  providerDef: Provider | null | undefined,
): DDNSValidationIssue | null => {
  const missingField = providerDef?.fields.find((field) => {
    if (field.required === false) {
      return false;
    }
    return !config[field.key]?.toString().trim();
  });

  if (!missingField) {
    return null;
  }

  return createValidationIssue("admin.ddns.fillField", {
    messageParams: { label: missingField.label },
  });
};

const DOMAIN_TARGET_ERROR_KEYS: Record<DDNSDomainTargetErrorCode, string> = {
  empty: "admin.ddns.invalidDomainTarget",
  invalid_domain: "admin.ddns.invalidDomainTarget",
  too_many_targets: "admin.ddns.tooManyDomainTargets",
  duplicate_targets: "admin.ddns.duplicateDomainTargets",
  invalid_pair: "admin.ddns.invalidDomainTargetPair",
  pair_unsupported: "admin.ddns.domainTargetPairUnsupported",
  root_mismatch: "admin.ddns.domainTargetRootMismatch",
};

export const validateProviderDomainTargetConfig = (
  config: Record<string, string>,
  providerDef: Provider | null | undefined,
): DDNSValidationIssue | null => {
  if (!providerDef?.fields.some((field) => field.key === "domain")) {
    return null;
  }

  const domain = config.domain ?? "";
  if (!domain || /^\p{White_Space}*$/u.test(domain)) {
    return null;
  }

  const capability = providerDef.capabilities?.domainTargets;
  const result = validateDDNSDomainTargets(domain, {
    capability,
    rootDomain: capability?.rootField
      ? config[capability.rootField]
      : undefined,
  });
  if (result.ok) {
    return null;
  }

  if (result.error === "root_mismatch" && capability?.rootField) {
    const rootField = providerDef.fields.find(
      (field) => field.key === capability.rootField,
    );
    return createValidationIssue(DOMAIN_TARGET_ERROR_KEYS[result.error], {
      messageParams: {
        rootField: rootField?.label || capability.rootField,
        rootDomain: config[capability.rootField]?.trim() || "-",
      },
    });
  }

  return createValidationIssue(DOMAIN_TARGET_ERROR_KEYS[result.error]);
};

const validateSingleAddressProviderScope = (
  providers: Provider[],
  providerName: string,
  updateScope: DDNSUpdateScope,
): DDNSValidationIssue | null => {
  if (
    isSingleAddressProvider(providers, providerName) &&
    updateScope === "dual_stack"
  ) {
    return createValidationIssue(
      "admin.ddns.singleAddressProviderRequiresSingleStack",
    );
  }

  return null;
};

const validateInterfaceIpSourceConfig = ({
  config,
  includeFilteredDescriptions,
  ipv4Options,
  ipv6Options,
  updateScope,
}: {
  config: Record<string, string>;
  includeFilteredDescriptions: boolean;
  ipv4Options: DDNSAddressOption[];
  ipv6Options: DDNSAddressOption[];
  updateScope: DDNSUpdateScope;
}): DDNSValidationIssue | null => {
  const networkInterface = normalizeNetworkInterface(
    config[NETWORK_INTERFACE_KEY],
  );

  if (!networkInterface) {
    return createValidationIssue("admin.ddns.chooseInterface", {
      descriptionKey: "admin.ddns.chooseInterfaceDescription",
    });
  }

  if (updateScope === "ipv4_only" && ipv4Options.length === 0) {
    return createValidationIssue("admin.ddns.noIpv4Available", {
      descriptionKey: includeFilteredDescriptions
        ? "admin.ddns.addressFilteredDescription"
        : undefined,
    });
  }

  if (updateScope === "ipv6_only" && ipv6Options.length === 0) {
    return createValidationIssue("admin.ddns.noIpv6Available", {
      descriptionKey: includeFilteredDescriptions
        ? "admin.ddns.addressFilteredDescription"
        : undefined,
    });
  }

  if (
    updateScope === "dual_stack" &&
    ipv4Options.length === 0 &&
    ipv6Options.length === 0
  ) {
    return createValidationIssue("admin.ddns.noAddressAvailable", {
      descriptionKey: includeFilteredDescriptions
        ? "admin.ddns.addressFilteredDescription"
        : undefined,
    });
  }

  const needsIPv4 = updateScope !== "ipv6_only";
  const needsIPv6 = updateScope !== "ipv4_only";
  const ipv4SelectorRaw = config[INTERFACE_IPV4_SELECTOR_KEY]?.trim() || "";
  const ipv6SelectorRaw = config[INTERFACE_IPV6_SELECTOR_KEY]?.trim() || "";
  const ipv4Selector = parseInterfaceSelector(ipv4SelectorRaw);
  const ipv6Selector = parseInterfaceSelector(ipv6SelectorRaw);

  if (
    (needsIPv4 && ipv4SelectorRaw && !ipv4Selector) ||
    (needsIPv6 && ipv6SelectorRaw && !ipv6Selector)
  ) {
    return createValidationIssue("admin.ddns.interfaceSelectorInvalid");
  }

  // An explicit selector is optional. The runtime's implicit auto selector
  // keeps the current address or deterministically ranks usable candidates.
  return null;
};

export const validateDDNSCommonConfig = ({
  config,
  ipSource,
  ipv4Options,
  ipv6Options,
  providerName,
  providers,
  updateScope,
}: {
  config: Record<string, string>;
  ipSource: DDNSIpSource;
  ipv4Options: DDNSAddressOption[];
  ipv6Options: DDNSAddressOption[];
  providerName: string;
  providers: Provider[];
  updateScope: DDNSUpdateScope;
}): DDNSValidationIssue | null => {
  const providerDef = findProviderDef(providers, providerName);
  const domainIssue = validateProviderDomainTargetConfig(config, providerDef);
  if (domainIssue) {
    return domainIssue;
  }

  const scopeIssue = validateSingleAddressProviderScope(
    providers,
    providerName,
    updateScope,
  );
  if (scopeIssue) {
    return scopeIssue;
  }

  if (ipSource === "static") {
    return validateStaticIpSourceConfig(config, updateScope);
  }

  if (ipSource === "domain") {
    return validateDomainIpSourceConfig(config);
  }

  if (ipSource !== "interface") {
    return null;
  }

  return validateInterfaceIpSourceConfig({
    config,
    includeFilteredDescriptions: true,
    ipv4Options,
    ipv6Options,
    updateScope,
  });
};

export const validateDDNSTargetConfig = ({
  config,
  ipv4Options,
  ipv6Options,
  provider,
  providerDef,
  providers,
  updateScope,
}: {
  config: Record<string, string>;
  ipv4Options: DDNSAddressOption[];
  ipv6Options: DDNSAddressOption[];
  provider: string;
  providerDef: Provider | null | undefined;
  providers: Provider[];
  updateScope: DDNSUpdateScope;
}): DDNSValidationIssue | null => {
  const providerName = provider.trim();

  if (!providerName) {
    return createValidationIssue("admin.ddns.selectProvider");
  }

  const fieldIssue = validateProviderConfigFields(config, providerDef);
  if (fieldIssue) {
    return fieldIssue;
  }

  const domainIssue = validateProviderDomainTargetConfig(config, providerDef);
  if (domainIssue) {
    return domainIssue;
  }

  const scopeIssue = validateSingleAddressProviderScope(
    providers,
    providerName,
    updateScope,
  );
  if (scopeIssue) {
    return scopeIssue;
  }

  const ipSource = normalizeIpSource(config[IP_SOURCE_KEY]);

  if (ipSource === "static") {
    return validateStaticIpSourceConfig(config, updateScope);
  }

  if (ipSource === "domain") {
    return validateDomainIpSourceConfig(config);
  }

  if (ipSource !== "interface") {
    return null;
  }

  return validateInterfaceIpSourceConfig({
    config,
    includeFilteredDescriptions: false,
    ipv4Options,
    ipv6Options,
    updateScope,
  });
};

export const parseUpdateIntervalDraft = (value: unknown) => {
  const trimmed = String(value ?? "").trim();
  if (!/^\d+$/.test(trimmed)) {
    return null;
  }

  const parsed = Number(trimmed);
  if (
    !Number.isInteger(parsed) ||
    parsed < MIN_DDNS_UPDATE_INTERVAL_MINUTES ||
    parsed > MAX_DDNS_UPDATE_INTERVAL_MINUTES
  ) {
    return null;
  }

  return parsed;
};
