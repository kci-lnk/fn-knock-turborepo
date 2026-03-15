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
  lastIP: DDNSLastIP;
  lastCheck: DDNSLastCheck;
};

export type DDNSProviderUpdater = (
  config: Record<string, string>,
  ipv4: string | null,
  ipv6: string | null
) => Promise<DDNSUpdateResult>;
