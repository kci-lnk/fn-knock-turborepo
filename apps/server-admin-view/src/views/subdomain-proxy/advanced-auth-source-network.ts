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

const isValueSeparator = (value: string) =>
  value === "," || value === "，" || value === "\r" || value === "\n";

/**
 * Parse the compact, CSV-like value editor without losing literal commas.
 * Values containing separators can be wrapped in double quotes; doubled
 * quotes inside a quoted value are decoded to one literal quote.
 */
export const parseAdvancedAuthValueList = (input: string): string[] => {
  const values: string[] = [];
  let buffer = "";
  let inQuotes = false;
  let quoted = false;
  let quoteClosed = false;

  const commit = () => {
    const value = quoted ? buffer : buffer.trim();
    if (value.length > 0) values.push(value);
    buffer = "";
    quoted = false;
    quoteClosed = false;
  };

  for (let index = 0; index < input.length; index += 1) {
    const character = input[index] ?? "";

    if (inQuotes) {
      if (character === '"') {
        if (input[index + 1] === '"') {
          buffer += '"';
          index += 1;
        } else {
          inQuotes = false;
          quoteClosed = true;
        }
      } else {
        buffer += character;
      }
      continue;
    }

    if (isValueSeparator(character)) {
      commit();
      if (character === "\r" && input[index + 1] === "\n") index += 1;
      continue;
    }

    if (character === '"' && buffer.trim().length === 0 && !quoted) {
      buffer = "";
      quoted = true;
      inQuotes = true;
      continue;
    }

    if (quoteClosed && /\s/u.test(character)) continue;
    quoteClosed = false;
    buffer += character;
  }

  commit();
  return values;
};

export const formatAdvancedAuthValueList = (
  values: readonly string[],
): string =>
  values
    .map((value) => {
      const requiresQuotes =
        /[",，\r\n]/u.test(value) || value.trim() !== value;
      return requiresQuotes ? `"${value.replace(/"/gu, '""')}"` : value;
    })
    .join(", ");

export const sourceNetworkInputKind = (
  operator: AdvancedAuthOperator,
): SourceNetworkInputKind =>
  operator === "equals" || operator === "not_equals" ? "address" : "cidr";

export const parseSourceNetworkTextarea = (value: string): string[] => {
  const values: string[] = [];
  const seen = new Set<string>();

  for (const raw of parseAdvancedAuthValueList(value)) {
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
