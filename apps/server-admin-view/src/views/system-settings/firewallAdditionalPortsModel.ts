import type { FirewallAdditionalPortsDetails } from "@/types";

export type FirewallPortRange =
  FirewallAdditionalPortsDetails["additionalRanges"][number];

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

export type FirewallPortRangeDraft = { start: string; end: string };
export const validateFirewallPortSelection = (
  values: readonly string[],
  rangeDrafts: readonly FirewallPortRangeDraft[],
):
  | { valid: true; ports: number[]; ranges: FirewallPortRange[] }
  | {
      valid: false;
      code: FirewallAdditionalPortValidationCode | "rangeOrder" | "overlap";
      index?: number;
    } => {
  if (values.length + rangeDrafts.length > MAX_FIREWALL_ADDITIONAL_PORTS) {
    return { valid: false, code: "tooMany" };
  }
  const single = validateFirewallAdditionalPortDraft(values);
  if (!single.valid) return single;
  const ranges: FirewallPortRange[] = [];
  for (const [index, draft] of rangeDrafts.entries()) {
    const start = validateFirewallAdditionalPortDraft([draft.start]);
    const end = validateFirewallAdditionalPortDraft([draft.end]);
    if (!start.valid) return { ...start, index };
    if (!end.valid) return { ...end, index };
    if (start.ports[0]! >= end.ports[0]!)
      return { valid: false, code: "rangeOrder", index };
    ranges.push({ start: start.ports[0]!, end: end.ports[0]! });
  }
  ranges.sort((a, b) => a.start - b.start || a.end - b.end);
  if (
    ranges.some(
      (range, index) =>
        (index > 0 && range.start <= ranges[index - 1]!.end) ||
        single.ports.some((port) => port >= range.start && port <= range.end),
    )
  ) {
    return { valid: false, code: "overlap" };
  }
  return { valid: true, ports: single.ports, ranges };
};

export const areFirewallRangeListsEqual = (
  left: readonly FirewallPortRange[],
  right: readonly FirewallPortRange[],
) => {
  const sort = (ranges: readonly FirewallPortRange[]) =>
    [...ranges].sort((a, b) => a.start - b.start || a.end - b.end);
  const sortedRight = sort(right);
  return (
    left.length === right.length &&
    sort(left).every(
      (range, index) =>
        range.start === sortedRight[index]!.start &&
        range.end === sortedRight[index]!.end,
    )
  );
};

export const formatFirewallPortSelection = (
  ports: number[],
  ranges: FirewallPortRange[],
  separator: string,
) =>
  [
    ...ports.map(String),
    ...ranges.map((range) => `${range.start}–${range.end}`),
  ].join(separator);
