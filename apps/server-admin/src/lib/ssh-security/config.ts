import { BlockList, isIP } from "node:net";
import {
  isValidCIDR,
  normalizeCidrLines,
} from "../../../../../packages/admin-shared/src/utils/cidr";
import type {
  SSHSecurityBlockDurationUnit,
  SSHSecurityConfig,
  SSHSecurityRuntimeState,
  SSHSecuritySelection,
} from "./types";
import { tDefault } from "../i18n";

const sshSecurityT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => tDefault(`server.sshSecurity.${key}`, params);

export const DEFAULT_SSH_SECURITY_CONFIG: SSHSecurityConfig = {
  enabled: false,
  window_minutes: 10,
  failed_login_threshold: 5,
  block_duration_value: 1,
  block_duration_unit: "day",
  allowed_regions: [],
  custom_cidrs: [],
  configured_at: null,
  updated_at: null,
};

export const DEFAULT_SSH_SECURITY_RUNTIME_STATE: SSHSecurityRuntimeState = {
  enabled: false,
  allowed_cidrs: [],
  updated_at: null,
};

export interface SSHSecuritySelectionInput {
  province: string;
  query_city?: string | null;
}

export interface SSHSecurityConfigInput {
  enabled?: boolean;
  window_minutes?: number;
  failed_login_threshold?: number;
  block_duration_value?: number;
  block_duration_unit?: SSHSecurityBlockDurationUnit;
  allowed_regions?: SSHSecuritySelectionInput[];
  custom_cidrs?: string[];
}

const normalizePositiveInt = (
  value: unknown,
  fallback: number,
  options: { min?: number; max?: number } = {},
): number => {
  const min = options.min ?? 1;
  const max = options.max ?? Number.MAX_SAFE_INTEGER;
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) return fallback;
  return Math.min(max, Math.max(min, parsed));
};

const normalizeTimestamp = (value: unknown): string | null => {
  if (typeof value !== "string") return null;
  const parsed = Date.parse(value);
  return Number.isFinite(parsed) ? new Date(parsed).toISOString() : null;
};

const normalizeString = (value: unknown): string => String(value ?? "").trim();

export const normalizeSSHSecurityDurationUnit = (
  value: unknown,
): SSHSecurityBlockDurationUnit => {
  if (value === "minute" || value === "hour" || value === "day") {
    return value;
  }
  return DEFAULT_SSH_SECURITY_CONFIG.block_duration_unit;
};

export const normalizeSSHSecuritySelection = (
  value: unknown,
): SSHSecuritySelection | null => {
  if (!value || typeof value !== "object") return null;
  const raw = value as Partial<SSHSecuritySelection>;
  const province = normalizeString(raw.province);
  const label = normalizeString(raw.label);
  const valueLabel = normalizeString(raw.value);
  if (!province || !label || !valueLabel) return null;

  const city = normalizeString(raw.city);
  const queryCity = normalizeString(raw.query_city);

  return {
    province,
    city: city || null,
    label,
    value: valueLabel,
    query_city: queryCity || null,
    is_province_wide: raw.is_province_wide === true,
    is_municipality: raw.is_municipality === true,
  };
};

export const normalizeSSHSecurityConfig = (
  value?: Partial<SSHSecurityConfig> | null,
): SSHSecurityConfig => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    window_minutes: normalizePositiveInt(
      raw.window_minutes,
      DEFAULT_SSH_SECURITY_CONFIG.window_minutes,
      { min: 1, max: 24 * 60 },
    ),
    failed_login_threshold: normalizePositiveInt(
      raw.failed_login_threshold,
      DEFAULT_SSH_SECURITY_CONFIG.failed_login_threshold,
      { min: 1, max: 1000 },
    ),
    block_duration_value: normalizePositiveInt(
      raw.block_duration_value,
      DEFAULT_SSH_SECURITY_CONFIG.block_duration_value,
      { min: 1, max: 365 },
    ),
    block_duration_unit: normalizeSSHSecurityDurationUnit(
      raw.block_duration_unit,
    ),
    allowed_regions: Array.isArray(raw.allowed_regions)
      ? raw.allowed_regions
          .map((item) => normalizeSSHSecuritySelection(item))
          .filter((item): item is SSHSecuritySelection => item !== null)
      : [],
    custom_cidrs: normalizeCidrLines(
      Array.isArray(raw.custom_cidrs)
        ? raw.custom_cidrs.map((item) => String(item ?? ""))
        : [],
    ),
    configured_at: normalizeTimestamp(raw.configured_at),
    updated_at: normalizeTimestamp(raw.updated_at),
  };
};

export const normalizeSSHSecurityRuntimeState = (
  value?: Partial<SSHSecurityRuntimeState> | null,
): SSHSecurityRuntimeState => {
  const raw = value ?? {};

  return {
    enabled: raw.enabled === true,
    allowed_cidrs: normalizeCidrLines(
      Array.isArray(raw.allowed_cidrs)
        ? raw.allowed_cidrs.map((item) => String(item ?? ""))
        : [],
    ),
    updated_at: normalizeTimestamp(raw.updated_at),
  };
};

export const validateSSHSecurityCustomCidrs = (value: string[]): string[] => {
  const normalized = normalizeCidrLines(value);
  const invalid = normalized.filter((cidr) => !isValidCIDR(cidr));
  if (invalid.length > 0) {
    throw new Error(
      sshSecurityT("customCidrInvalid", { cidrs: invalid.join(", ") }),
    );
  }
  return normalized;
};

export const sshSecurityDurationToSeconds = (
  config: Pick<
    SSHSecurityConfig,
    "block_duration_value" | "block_duration_unit"
  >,
): number => {
  const value = normalizePositiveInt(config.block_duration_value, 1, {
    min: 1,
    max: 365,
  });
  const unit = normalizeSSHSecurityDurationUnit(config.block_duration_unit);
  if (unit === "minute") return value * 60;
  if (unit === "hour") return value * 3600;
  return value * 24 * 3600;
};

export const buildCIDRMatcher = (cidrs: string[]) => {
  const blockList = new BlockList();
  for (const cidr of normalizeCidrLines(cidrs)) {
    const [address, prefixText] = cidr.split("/");
    const prefix = Number.parseInt(prefixText ?? "", 10);
    if (!address || !Number.isFinite(prefix)) continue;
    const family = isIP(address) === 6 ? "ipv6" : "ipv4";
    blockList.addSubnet(address, prefix, family);
  }

  return {
    contains(ip: string): boolean {
      const family = isIP(ip) === 6 ? "ipv6" : "ipv4";
      return blockList.check(ip, family);
    },
  };
};
