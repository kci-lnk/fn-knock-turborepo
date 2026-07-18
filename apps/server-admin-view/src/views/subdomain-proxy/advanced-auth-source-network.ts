import {
  isValidCIDR,
  isValidIPv4Address,
  isValidIPv6Address,
} from "@admin-shared/utils/cidr";
import type { AdvancedAuthOperator } from "@/types";

export type SourceNetworkInputKind = "address" | "cidr";

export interface SourceNetworkValidationIssue {
  kind: SourceNetworkInputKind;
  line: number;
}

export const sourceNetworkInputKind = (
  operator: AdvancedAuthOperator,
): SourceNetworkInputKind =>
  operator === "equals" || operator === "not_equals" ? "address" : "cidr";

export const parseSourceNetworkTextarea = (value: string): string[] => {
  const values: string[] = [];
  const seen = new Set<string>();

  for (const raw of value.split(/[\r\n,，]+/u)) {
    const normalized = raw.trim();
    if (!normalized) continue;
    const key = normalized.toLowerCase();
    if (seen.has(key)) continue;
    seen.add(key);
    values.push(normalized);
  }

  return values;
};

export const getSourceNetworkValidationIssue = (
  values: readonly string[],
  operator: AdvancedAuthOperator,
): SourceNetworkValidationIssue | null => {
  const kind = sourceNetworkInputKind(operator);
  for (const [index, raw] of values.entries()) {
    const value = raw.trim();
    const valid =
      kind === "cidr"
        ? isValidCIDR(value)
        : isValidIPv4Address(value) || isValidIPv6Address(value);
    if (!valid) return { kind, line: index + 1 };
  }
  return null;
};
