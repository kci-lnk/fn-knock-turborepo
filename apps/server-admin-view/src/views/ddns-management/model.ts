import type {
  DDNSIpSource,
  DDNSNetworkInterfacePayload,
  DDNSTargetSummaryPayload,
  DDNSUpdateScope,
} from "@/lib/api";

export type { DDNSIpSource, DDNSUpdateScope } from "@/lib/api";

export interface ProviderField {
  key: string;
  label: string;
  type: "text" | "password" | "select";
  placeholder?: string;
  required?: boolean;
  options?: { label: string; value: string }[];
  description?: string;
}

export interface Provider {
  name: string;
  label: string;
  fields: ProviderField[];
  capabilities?: {
    addressMode?: "dual_stack" | "single_address";
    ipSources?: DDNSIpSource[];
  };
}

export interface LogEntry {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
}

export interface LastIP {
  ipv4: string | null;
  ipv6: string | null;
  updated_at: string | null;
}

export interface LastCheck {
  checked_at: string | null;
  outcome: "updated" | "noop" | "skipped" | "error" | null;
  message: string | null;
}

export interface TargetDialogState {
  id: string | null;
  name: string;
  enabled: boolean;
  provider: string;
  config: Record<string, string>;
}

export const UPDATE_SCOPE_KEY = "update_scope";
export const IP_SOURCE_KEY = "ip_source";
export const NETWORK_INTERFACE_KEY = "network_interface";
export const INTERFACE_IPV4_INDEX_KEY = "interface_ipv4_index";
export const INTERFACE_IPV6_INDEX_KEY = "interface_ipv6_index";
export const STATIC_IPV4_KEY = "static_ipv4";
export const STATIC_IPV6_KEY = "static_ipv6";
export const SOURCE_DOMAIN_KEY = "source_domain";
export const NETWORK_INTERFACE_AUTO_VALUE = "__auto__";
export const DEFAULT_DDNS_UPDATE_SCOPE: DDNSUpdateScope = "dual_stack";
export const DEFAULT_DDNS_IP_SOURCE: DDNSIpSource = "public";
export const DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES = 10;
export const MIN_DDNS_UPDATE_INTERVAL_MINUTES = 5;
export const MAX_DDNS_UPDATE_INTERVAL_MINUTES = 1440;

export const UPDATE_SCOPE_OPTIONS: Array<{
  labelKey: string;
  value: DDNSUpdateScope;
}> = [
  { labelKey: "admin.ddns.updateScope.dualStack", value: "dual_stack" },
  { labelKey: "admin.ddns.updateScope.ipv6Only", value: "ipv6_only" },
  { labelKey: "admin.ddns.updateScope.ipv4Only", value: "ipv4_only" },
];

export const IP_SOURCE_OPTIONS: Array<{
  labelKey: string;
  value: DDNSIpSource;
}> = [
  { labelKey: "admin.ddns.ipSource.public", value: "public" },
  { labelKey: "admin.ddns.ipSource.interface", value: "interface" },
  { labelKey: "admin.ddns.ipSource.static", value: "static" },
  { labelKey: "admin.ddns.ipSource.domain", value: "domain" },
];

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

export const normalizeNetworkInterface = (value: string | null | undefined) =>
  value?.trim() || "";

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
    });
  }
  return resolved;
};

export const findProviderDef = (
  providers: Provider[],
  providerName: string,
): Provider | null =>
  providers.find((provider) => provider.name === providerName) || null;

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
) =>
  (option?.selectableAddresses || [])
    .filter((item) => item.family === family)
    .map((item, index) => ({
      value: String(index),
      label: getLabel(item, index),
    }));

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
  includeUnavailableSelectionChecks,
  ipv4Options,
  ipv6Options,
  updateScope,
}: {
  config: Record<string, string>;
  includeFilteredDescriptions: boolean;
  includeUnavailableSelectionChecks: boolean;
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
  const ipv4Index = normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV4_INDEX_KEY],
  );
  const ipv6Index = normalizeInterfaceAddressIndex(
    config[INTERFACE_IPV6_INDEX_KEY],
  );

  if (needsIPv4 && ipv4Options.length > 0 && !ipv4Index) {
    return createValidationIssue("admin.ddns.chooseIpv4", {
      descriptionKey: includeFilteredDescriptions
        ? "admin.ddns.chooseIpv4Description"
        : undefined,
    });
  }

  if (
    includeUnavailableSelectionChecks &&
    needsIPv4 &&
    ipv4Index &&
    !ipv4Options.some((option) => option.value === ipv4Index)
  ) {
    return createValidationIssue("admin.ddns.ipv4Unavailable", {
      descriptionKey: "admin.ddns.ipv4UnavailableDescription",
    });
  }

  if (needsIPv6 && ipv6Options.length > 0 && !ipv6Index) {
    return createValidationIssue("admin.ddns.chooseIpv6", {
      descriptionKey: includeFilteredDescriptions
        ? "admin.ddns.chooseIpv6Description"
        : undefined,
    });
  }

  if (
    includeUnavailableSelectionChecks &&
    needsIPv6 &&
    ipv6Index &&
    !ipv6Options.some((option) => option.value === ipv6Index)
  ) {
    return createValidationIssue("admin.ddns.ipv6Unavailable", {
      descriptionKey: "admin.ddns.ipv6UnavailableDescription",
    });
  }

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
    includeUnavailableSelectionChecks: true,
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
    includeUnavailableSelectionChecks: false,
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
