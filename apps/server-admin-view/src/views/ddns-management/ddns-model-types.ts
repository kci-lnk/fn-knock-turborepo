import type {
  DDNSHttpTransport,
  DDNSIpSource,
  DDNSProviderCapabilities,
  DDNSPublicCheckSourcesPayload,
  DDNSPublicDnsProvider,
  DDNSUpdateScope,
} from "@/lib/api/ddns";

export type {
  DDNSHttpTransport,
  DDNSIpSource,
  DDNSPublicDnsProvider,
  DDNSUpdateScope,
} from "@/lib/api/ddns";

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
  capabilities?: DDNSProviderCapabilities;
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
  lastIP?: {
    ipv4: string | null;
    ipv6: string | null;
  };
  selectionAnchor?: {
    ipv4: string | null;
    ipv6: string | null;
  };
}

export const UPDATE_SCOPE_KEY = "update_scope";
export const IP_SOURCE_KEY = "ip_source";
export const NETWORK_INTERFACE_KEY = "network_interface";
export const INTERFACE_IPV4_INDEX_KEY = "interface_ipv4_index";
export const INTERFACE_IPV6_INDEX_KEY = "interface_ipv6_index";
export const INTERFACE_IPV4_SELECTOR_KEY = "interface_ipv4_selector";
export const INTERFACE_IPV6_SELECTOR_KEY = "interface_ipv6_selector";
export const ALLOW_PRIVATE_ADDRESSES_KEY = "allow_private_addresses";
export const STATIC_IPV4_KEY = "static_ipv4";
export const STATIC_IPV6_KEY = "static_ipv6";
export const SOURCE_DOMAIN_KEY = "source_domain";
export const NETWORK_INTERFACE_AUTO_VALUE = "__auto__";
export const DEFAULT_DDNS_UPDATE_SCOPE: DDNSUpdateScope = "dual_stack";
export const DEFAULT_DDNS_IP_SOURCE: DDNSIpSource = "public";
export const DEFAULT_DDNS_HTTP_TRANSPORT: DDNSHttpTransport = "node";
export const DEFAULT_DDNS_PUBLIC_DNS_PROVIDER: DDNSPublicDnsProvider = "alidns";
export const DEFAULT_DDNS_UPDATE_INTERVAL_MINUTES = 10;
export const EMPTY_DDNS_PUBLIC_CHECK_SOURCES: DDNSPublicCheckSourcesPayload = {
  ipv4: [],
  ipv6: [],
};
export const MIN_DDNS_UPDATE_INTERVAL_MINUTES = 2;
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

export const HTTP_TRANSPORT_OPTIONS: Array<{
  labelKey: string;
  value: DDNSHttpTransport;
}> = [
  { labelKey: "admin.ddns.httpTransport.node", value: "node" },
  { labelKey: "admin.ddns.httpTransport.curl", value: "curl" },
];

export const PUBLIC_DNS_PROVIDER_OPTIONS: Array<{
  labelKey: string;
  value: DDNSPublicDnsProvider;
}> = [
  { labelKey: "admin.ddns.publicDnsProvider.none", value: "none" },
  { labelKey: "admin.ddns.publicDnsProvider.alidns", value: "alidns" },
  { labelKey: "admin.ddns.publicDnsProvider.tencent", value: "tencent" },
  {
    labelKey: "admin.ddns.publicDnsProvider.cloudflare",
    value: "cloudflare",
  },
  { labelKey: "admin.ddns.publicDnsProvider.google", value: "google" },
];
