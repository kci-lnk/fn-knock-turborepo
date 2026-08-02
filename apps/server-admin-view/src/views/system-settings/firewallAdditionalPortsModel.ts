import type { FirewallAdditionalPortsDetails } from "@/types";

export const MAX_FIREWALL_ADDITIONAL_PORTS = 128;

export type FirewallAdditionalPortsSuccessMessageKey =
  | "savedAndAppliedDescription"
  | "savedForLaterDescription"
  | "savedForLaterManualDescription";

export const resolveFirewallAdditionalPortsSuccessMessageKey = (
  result: Pick<FirewallAdditionalPortsDetails, "appliedNow">,
  autoManageFirewallEnabled: boolean,
): FirewallAdditionalPortsSuccessMessageKey => {
  if (result.appliedNow) return "savedAndAppliedDescription";
  return autoManageFirewallEnabled
    ? "savedForLaterDescription"
    : "savedForLaterManualDescription";
};

export type FirewallAdditionalPortValidationCode =
  "required" | "integer" | "range" | "duplicate" | "tooMany";

export type FirewallAdditionalPortValidation =
  | { valid: true; ports: number[] }
  | {
      valid: false;
      code: FirewallAdditionalPortValidationCode;
      index?: number;
    };

export const validateFirewallAdditionalPortDraft = (
  values: readonly string[],
): FirewallAdditionalPortValidation => {
  if (values.length > MAX_FIREWALL_ADDITIONAL_PORTS) {
    return { valid: false, code: "tooMany" };
  }
  const ports: number[] = [];
  const seen = new Set<number>();
  for (const [index, value] of values.entries()) {
    const normalized = value.trim();
    if (!normalized) return { valid: false, code: "required", index };
    if (!/^\d+$/u.test(normalized)) {
      return { valid: false, code: "integer", index };
    }
    const port = Number(normalized);
    if (!Number.isSafeInteger(port)) {
      return { valid: false, code: "integer", index };
    }
    if (port < 1 || port > 65535) {
      return { valid: false, code: "range", index };
    }
    if (seen.has(port)) {
      return { valid: false, code: "duplicate", index };
    }
    seen.add(port);
    ports.push(port);
  }
  return { valid: true, ports: ports.sort((left, right) => left - right) };
};

export const areFirewallPortListsEqual = (
  left: readonly number[],
  right: readonly number[],
) => {
  if (left.length !== right.length) return false;
  const sortedLeft = [...left].sort((a, b) => a - b);
  const sortedRight = [...right].sort((a, b) => a - b);
  return sortedLeft.every((port, index) => port === sortedRight[index]);
};
