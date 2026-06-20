import type { DDNSLastCheck, DDNSLastIP } from "./types";

export const buildEmptyDDNSLastIP = (): DDNSLastIP => ({
  ipv4: null,
  ipv6: null,
  updated_at: null,
});

export const buildEmptyDDNSLastCheck = (): DDNSLastCheck => ({
  checked_at: null,
  outcome: null,
  message: null,
});

const normalizeDDNSLastCheckOutcome = (
  value: string | null | undefined,
): DDNSLastCheck["outcome"] => {
  return value === "updated" ||
    value === "noop" ||
    value === "skipped" ||
    value === "error"
    ? value
    : null;
};

export const parseDDNSLastIPHash = (
  data: Record<string, string> | null | undefined,
): DDNSLastIP => ({
  ipv4: data?.ipv4 || null,
  ipv6: data?.ipv6 || null,
  updated_at: data?.updated_at || null,
});

export const parseDDNSLastCheckHash = (
  data: Record<string, string> | null | undefined,
): DDNSLastCheck => ({
  checked_at: data?.checked_at || null,
  outcome: normalizeDDNSLastCheckOutcome(data?.outcome),
  message: data?.message || null,
});

export const serializeDDNSLastIPHash = (
  status: DDNSLastIP,
): Record<string, string> => {
  const payload: Record<string, string> = {};

  if (status.ipv4) {
    payload.ipv4 = status.ipv4;
  }
  if (status.ipv6) {
    payload.ipv6 = status.ipv6;
  }
  if (status.updated_at) {
    payload.updated_at = status.updated_at;
  }

  return payload;
};

export const serializeDDNSLastCheckHash = (
  status: DDNSLastCheck,
): Record<string, string> => {
  const payload: Record<string, string> = {};

  if (status.checked_at) {
    payload.checked_at = status.checked_at;
  }
  if (status.outcome) {
    payload.outcome = status.outcome;
  }
  if (status.message) {
    payload.message = status.message;
  }

  return payload;
};
