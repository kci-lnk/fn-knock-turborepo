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
};

export type DDNSNetworkInterfaceAddress = {
  family: "ipv4" | "ipv6";
  address: string;
  cidr: string | null;
  internal: boolean;
};

export type DDNSNetworkInterfaceOption = {
  name: string;
  label: string;
  summary: string;
  hasIpv4: boolean;
  hasIpv6: boolean;
  addresses: DDNSNetworkInterfaceAddress[];
};

export type DDNSUpdateResult = {
  success: boolean;
  message: string;
  ipv4Updated?: boolean;
  ipv6Updated?: boolean;
};

export type DDNSUpdateScope = "dual_stack" | "ipv6_only" | "ipv4_only";

export type DDNSLogEntry = {
  time: string;
  level: "info" | "error" | "warn";
  message: string;
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
  updateScope: DDNSUpdateScope;
  networkInterface: string;
  lastIP: DDNSLastIP;
  lastCheck: DDNSLastCheck;
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
  ipv6: string | null
) => Promise<DDNSUpdateResult>;
