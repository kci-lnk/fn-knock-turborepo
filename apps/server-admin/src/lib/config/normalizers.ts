import { normalizeCidrLines } from "../../../../../packages/admin-shared/src/utils/cidr";

import { DEFAULT_WAF_CONFIG } from "./defaults";
import type { ScanDiscoveryConfig, WAFConfig, WAFMode } from "./types";

export const normalizeStringList = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];
  const items: string[] = [];
  const seen = new Set<string>();
  for (const raw of value) {
    const item = String(raw ?? "").trim();
    if (!item || seen.has(item)) continue;
    seen.add(item);
    items.push(item);
  }
  return items;
};

export const normalizeOptionalString = (value: unknown): string | undefined => {
  if (typeof value !== "string") return undefined;
  const normalized = value.trim();
  return normalized || undefined;
};

export const normalizePositiveInt = (
  value: unknown,
  fallback: number,
  {
    min = 1,
    max = Number.MAX_SAFE_INTEGER,
  }: { min?: number; max?: number } = {},
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
};

export const normalizeBoundedInt = (
  value: unknown,
  fallback: number,
  {
    min = 0,
    max = Number.MAX_SAFE_INTEGER,
  }: { min?: number; max?: number } = {},
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
};

const normalizeWAFMode = (value: unknown): WAFMode => {
  if (value === "off" || value === "detection" || value === "blocking") {
    return value;
  }
  return DEFAULT_WAF_CONFIG.mode;
};

const normalizeParanoiaLevel = (value: unknown, fallback: 1 | 2 | 3 | 4) =>
  normalizeBoundedInt(value, fallback, { min: 1, max: 4 }) as 1 | 2 | 3 | 4;

const normalizePathPrefixList = (value: unknown): string[] => {
  const normalized: string[] = [];
  const seen = new Set<string>();
  for (const item of normalizeStringList(value)) {
    let prefix = item.trim();
    if (!prefix) continue;
    if (!prefix.startsWith("/")) prefix = `/${prefix}`;
    prefix = prefix.replace(/\/{2,}/g, "/");
    if (prefix.length > 1) prefix = prefix.replace(/\/+$/, "");
    if (!prefix || seen.has(prefix)) continue;
    seen.add(prefix);
    normalized.push(prefix);
  }
  return normalized;
};

export const normalizeWAFConfig = (
  value?: Partial<WAFConfig> | null,
): WAFConfig => {
  const raw = value ?? {};
  const paranoiaLevel = normalizeParanoiaLevel(
    raw.paranoia_level,
    DEFAULT_WAF_CONFIG.paranoia_level,
  );
  const executingParanoiaLevel = Math.max(
    paranoiaLevel,
    normalizeParanoiaLevel(
      raw.executing_paranoia_level,
      raw.paranoia_level
        ? paranoiaLevel
        : DEFAULT_WAF_CONFIG.executing_paranoia_level,
    ),
  ) as 1 | 2 | 3 | 4;
  const requestBodyLimit = normalizePositiveInt(
    raw.request_body_limit_bytes,
    DEFAULT_WAF_CONFIG.request_body_limit_bytes,
    { min: 1024, max: 128 * 1024 * 1024 },
  );
  const requestBodyMemoryLimit = normalizePositiveInt(
    raw.request_body_in_memory_limit_bytes,
    Math.min(
      DEFAULT_WAF_CONFIG.request_body_in_memory_limit_bytes,
      requestBodyLimit,
    ),
    { min: 1024, max: requestBodyLimit },
  );
  return {
    enabled: raw.enabled === true,
    system_rules_auto_update_enabled:
      typeof raw.system_rules_auto_update_enabled === "boolean"
        ? raw.system_rules_auto_update_enabled
        : DEFAULT_WAF_CONFIG.system_rules_auto_update_enabled,
    common_location_exempt_enabled: raw.common_location_exempt_enabled === true,
    mode: normalizeWAFMode(raw.mode),
    active_bundle_id: "local",
    rules_dir: DEFAULT_WAF_CONFIG.rules_dir,
    paranoia_level: paranoiaLevel,
    executing_paranoia_level: executingParanoiaLevel,
    inbound_anomaly_threshold: normalizePositiveInt(
      raw.inbound_anomaly_threshold,
      DEFAULT_WAF_CONFIG.inbound_anomaly_threshold,
      { min: 1, max: 1000000 },
    ),
    outbound_anomaly_threshold: normalizePositiveInt(
      raw.outbound_anomaly_threshold,
      DEFAULT_WAF_CONFIG.outbound_anomaly_threshold,
      { min: 1, max: 1000000 },
    ),
    request_body_access:
      typeof raw.request_body_access === "boolean"
        ? raw.request_body_access
        : DEFAULT_WAF_CONFIG.request_body_access,
    request_body_limit_bytes: requestBodyLimit,
    request_body_in_memory_limit_bytes: requestBodyMemoryLimit,
    response_body_access: false,
    disabled_hosts: normalizeStringList(raw.disabled_hosts).map((host) =>
      host.toLowerCase(),
    ),
    disabled_path_prefixes: normalizePathPrefixList(raw.disabled_path_prefixes),
    log_retention_days: normalizePositiveInt(
      raw.log_retention_days,
      DEFAULT_WAF_CONFIG.log_retention_days,
      { min: 1, max: 365 },
    ),
    drain_interval_seconds: normalizePositiveInt(
      raw.drain_interval_seconds,
      DEFAULT_WAF_CONFIG.drain_interval_seconds,
      { min: 1, max: 60 },
    ),
    updated_at: normalizeOptionalString(raw.updated_at) ?? null,
  };
};

export const normalizeScanDiscoveryConfig = (
  value?: Partial<ScanDiscoveryConfig> | null,
): ScanDiscoveryConfig => {
  const raw = value ?? {};

  return {
    custom_cidrs: normalizeCidrLines(
      Array.isArray(raw.custom_cidrs)
        ? raw.custom_cidrs.map((cidr) => String(cidr))
        : [],
    ),
    selected_cidrs: normalizeCidrLines(
      Array.isArray(raw.selected_cidrs)
        ? raw.selected_cidrs.map((cidr) => String(cidr))
        : [],
    ),
  };
};
