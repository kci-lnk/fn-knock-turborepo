export type DDNSUpdateScope = "dual_stack" | "ipv6_only" | "ipv4_only";
export type DDNSIpSource = "public" | "interface" | "static" | "domain";
export type DDNSProviderAddressMode = "dual_stack" | "single_address";

export type DDNSProviderCapabilities = {
  addressMode?: DDNSProviderAddressMode;
  ipSources?: DDNSIpSource[];
};

export type DDNSProviderField = {
  key: string;
  label: string;
  type: "text" | "password" | "select";
  placeholder?: string;
  required?: boolean;
  options?: { label: string; value: string }[];
  description?: string;
};

export type DDNSProviderDefinition = {
  name: string;
  label: string;
  fields: DDNSProviderField[];
  capabilities?: DDNSProviderCapabilities;
};

export type DDNSNetworkInterfaceAddress = {
  family: "ipv4" | "ipv6";
  address: string;
  cidr: string | null;
  internal: boolean;
  source?: "runtime" | "docker_host";
};

export type DDNSNetworkInterfaceOption = {
  name: string;
  label: string;
  summary: string;
  source?: "runtime" | "docker_host";
  hasIpv4: boolean;
  hasIpv6: boolean;
  addresses: DDNSNetworkInterfaceAddress[];
  selectableAddresses: DDNSNetworkInterfaceAddress[];
};

export type DDNSUpdateResult = {
  success: boolean;
  message: string;
  ipv4Updated?: boolean;
  ipv6Updated?: boolean;
};

export type DDNSPublicCheckFamily = "ipv4" | "ipv6";

export type DDNSPublicCheckSources = Record<DDNSPublicCheckFamily, string[]>;

export type DDNSPublicCheckTestResult = {
  family: DDNSPublicCheckFamily;
  url: string;
  success: boolean;
  status: number | null;
  ip: string | null;
  responsePreview?: string;
  error?: string;
};

export type DDNSStoredSettings = {
  updateIntervalMinutes: number;
  publicCheckSources: DDNSPublicCheckSources;
};

export type DDNSSettings = DDNSStoredSettings & {
  defaultPublicCheckSources: DDNSPublicCheckSources;
};

export type DDNSLogEntry = {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
  targetId?: string;
  targetName?: string;
  provider?: string | null;
  isPrimary?: boolean;
};

export type DDNSLastIP = {
  ipv4: string | null;
  ipv6: string | null;
  updated_at: string | null;
};

export type DDNSLastCheck = {
  checked_at: string | null;
  outcome: "updated" | "noop" | "skipped" | "error" | null;
  message: string | null;
};

export type DDNSStatus = {
  enabled: boolean;
  provider: string | null;
  updateIntervalMinutes: number;
  publicCheckSources: DDNSPublicCheckSources;
  defaultPublicCheckSources: DDNSPublicCheckSources;
  updateScope: DDNSUpdateScope;
  ipSource: DDNSIpSource;
  networkInterface: string;
  lastIP: DDNSLastIP;
  lastCheck: DDNSLastCheck;
  primaryTargetId: string | null;
  extraTargetCount: number;
  enabledExtraTargetCount: number;
  targets: DDNSTargetSummary[];
};

export type DDNSTargetMeta = {
  id: string;
  name: string;
  isPrimary: boolean;
  enabled: boolean;
  provider: string | null;
  createdAt: string;
  updatedAt: string;
  sortOrder: number;
};

export type DDNSTargetRuntimeStatus = {
  lastIP: DDNSLastIP;
  lastCheck: DDNSLastCheck;
};

export type DDNSTargetRecord = DDNSTargetMeta &
  DDNSTargetRuntimeStatus & {
    config: Record<string, string>;
  };

export type DDNSTargetSummary = DDNSTargetMeta &
  DDNSTargetRuntimeStatus & {
    updateScope: DDNSUpdateScope;
    providerLabel: string;
    domainSummary: string;
  };

export type DDNSTargetList = {
  primaryTargetId: string | null;
  total: number;
  extraCount: number;
  enabledExtraCount: number;
  items: DDNSTargetSummary[];
};

export type DDNSHttpClient = {
  fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>;
};

export type DDNSProviderContext = {
  config: Record<string, string>;
  http: DDNSHttpClient;
};

export type DDNSProviderUpdater = (
  context: DDNSProviderContext,
  ipv4: string | null,
  ipv6: string | null,
) => Promise<DDNSUpdateResult>;
