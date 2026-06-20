import { normalizeIp } from "../ip-normalize";
import {
  inferWhiteListTargetType,
  normalizeWhiteListTarget,
  type WhiteListTargetType,
} from "../whitelist-target";

export interface WhiteListRecord {
  id: string;
  ip: string;
  targetType: WhiteListTargetType;
  expireAt: number | null;
  source: "manual" | "auto";
  createdAt: number;
  comment?: string;
  status: "active" | "expired" | "deleted";
  ipLocation?: string;
  resolvedTargets?: string[];
  checkIntervalMinutes?: number | null;
  lastCheckedAt?: number | null;
  lastResolvedAt?: number | null;
  resolveStatus?: "pending" | "resolved" | "empty" | "error";
  resolveMessage?: string;
}

export interface WhiteListConcreteTargetRecord {
  recordId: string;
  recordTarget: string;
  recordTargetType: WhiteListTargetType;
  source: WhiteListRecord["source"];
  target: string;
  targetType: "ip" | "cidr";
}

const DEFAULT_CNAME_CHECK_INTERVAL_MINUTES = 5;
const MIN_CNAME_CHECK_INTERVAL_MINUTES = 1;
const MAX_CNAME_CHECK_INTERVAL_MINUTES = 24 * 60;

export const getRecordTargetType = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): WhiteListTargetType =>
  record.targetType === "cidr"
    ? "cidr"
    : record.targetType === "cname"
      ? "cname"
      : "ip";

export const getRecordTarget = (
  record: Partial<Pick<WhiteListRecord, "ip">>,
): string => String(record.ip || "").trim();

export const isIPRecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "ip";

export const isCIDRRecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "cidr";

export const isCNAMERecord = (
  record: Partial<Pick<WhiteListRecord, "targetType">>,
): boolean => getRecordTargetType(record) === "cname";

export const normalizeCnameCheckIntervalMinutes = (
  value: unknown,
): number => {
  const parsed = Number.parseInt(String(value ?? ""), 10);
  if (!Number.isFinite(parsed)) {
    return DEFAULT_CNAME_CHECK_INTERVAL_MINUTES;
  }

  return Math.min(
    MAX_CNAME_CHECK_INTERVAL_MINUTES,
    Math.max(MIN_CNAME_CHECK_INTERVAL_MINUTES, Math.floor(parsed)),
  );
};

export const normalizeResolvedTargets = (value: unknown): string[] => {
  if (!Array.isArray(value)) return [];

  const targets = new Set<string>();
  for (const item of value) {
    const normalized = normalizeIp(String(item ?? "").trim());
    if (!normalized) continue;
    targets.add(normalized);
  }

  return [...targets].sort((left, right) => left.localeCompare(right));
};

export const toOptionalTimestamp = (value: unknown): number | null => {
  if (value === null || value === undefined || value === "") {
    return null;
  }

  const parsed = Number.parseInt(String(value), 10);
  return Number.isFinite(parsed) ? parsed : null;
};

export const getCnameResolvedTargets = (
  record: Partial<Pick<WhiteListRecord, "resolvedTargets" | "targetType">>,
): string[] =>
  isCNAMERecord(record) ? normalizeResolvedTargets(record.resolvedTargets) : [];

export const getConcreteIPTargets = (
  record: Partial<
    Pick<WhiteListRecord, "ip" | "resolvedTargets" | "targetType">
  >,
): string[] => {
  if (isIPRecord(record)) {
    const normalized = normalizeIp(getRecordTarget(record));
    return normalized ? [normalized] : [];
  }

  if (isCNAMERecord(record)) {
    return getCnameResolvedTargets(record);
  }

  return [];
};

export const getConcreteTargets = (
  record: Partial<
    Pick<WhiteListRecord, "ip" | "resolvedTargets" | "targetType">
  >,
): Array<{ target: string; targetType: "ip" | "cidr" }> => {
  if (isCIDRRecord(record)) {
    const target = getRecordTarget(record);
    return target ? [{ target, targetType: "cidr" }] : [];
  }

  return getConcreteIPTargets(record).map((target) => ({
    target,
    targetType: "ip" as const,
  }));
};

export const sortRecordsByCreatedAtDesc = (
  records: WhiteListRecord[],
): WhiteListRecord[] =>
  records.sort((left, right) => right.createdAt - left.createdAt);

export const deserializeRecord = (raw: string): WhiteListRecord | null => {
  try {
    const parsed = JSON.parse(raw) as Partial<WhiteListRecord>;
    const id = String(parsed.id || "").trim();
    if (!id) return null;

    const rawTarget = getRecordTarget(parsed);
    const targetType =
      parsed.targetType === "cidr"
        ? "cidr"
        : parsed.targetType === "cname"
          ? "cname"
          : (inferWhiteListTargetType(rawTarget) ?? "ip");
    const normalizedTarget = normalizeWhiteListTarget(rawTarget, targetType);
    if (!normalizedTarget) return null;

    const source = parsed.source === "auto" ? "auto" : "manual";
    const status =
      parsed.status === "expired" || parsed.status === "deleted"
        ? parsed.status
        : "active";
    const createdAt = Number.parseInt(String(parsed.createdAt ?? 0), 10);
    const expireAt = toOptionalTimestamp(parsed.expireAt);
    const comment =
      typeof parsed.comment === "string" ? parsed.comment : undefined;
    const ipLocation =
      targetType === "ip" && typeof parsed.ipLocation === "string"
        ? parsed.ipLocation
        : undefined;
    const resolvedTargets =
      targetType === "cname"
        ? normalizeResolvedTargets(parsed.resolvedTargets)
        : undefined;
    const checkIntervalMinutes =
      targetType === "cname"
        ? normalizeCnameCheckIntervalMinutes(parsed.checkIntervalMinutes)
        : null;
    const lastCheckedAt = toOptionalTimestamp(parsed.lastCheckedAt);
    const lastResolvedAt = toOptionalTimestamp(parsed.lastResolvedAt);
    const resolveStatus =
      parsed.resolveStatus === "resolved" ||
      parsed.resolveStatus === "empty" ||
      parsed.resolveStatus === "error" ||
      parsed.resolveStatus === "pending"
        ? parsed.resolveStatus
        : targetType === "cname"
          ? "pending"
          : undefined;
    const resolveMessage =
      typeof parsed.resolveMessage === "string"
        ? parsed.resolveMessage.trim() || undefined
        : undefined;

    return {
      id,
      ip: normalizedTarget,
      targetType,
      expireAt:
        expireAt !== null && Number.isFinite(expireAt) ? expireAt : null,
      source,
      createdAt: Number.isFinite(createdAt) ? createdAt : 0,
      ...(comment !== undefined ? { comment } : {}),
      status,
      ...(ipLocation ? { ipLocation } : {}),
      ...(resolvedTargets !== undefined ? { resolvedTargets } : {}),
      ...(checkIntervalMinutes !== null ? { checkIntervalMinutes } : {}),
      ...(lastCheckedAt !== null ? { lastCheckedAt } : {}),
      ...(lastResolvedAt !== null ? { lastResolvedAt } : {}),
      ...(resolveStatus ? { resolveStatus } : {}),
      ...(resolveMessage ? { resolveMessage } : {}),
    };
  } catch {
    return null;
  }
};
