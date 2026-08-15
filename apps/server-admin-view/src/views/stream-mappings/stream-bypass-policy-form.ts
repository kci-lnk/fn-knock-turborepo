import type { StreamBypassPolicy } from "../../lib/api/config";
import type { GatewayVisibilitySelection } from "../../types";
import {
  CIDR_OPERATORS,
  getCidrRegionSelectionKey,
  getCidrRegionSelectionLabel,
  type CidrOperator,
} from "../../types/cidr";
import {
  formatAdvancedAuthValueList,
  getSourceNetworkValidationIssue,
  parseSourceNetworkTextarea,
  sourceNetworkInputKind,
} from "../subdomain-proxy/advanced-auth-source-network";

export const MAX_STREAM_BYPASS_GROUPS = 16;
export const MAX_STREAM_BYPASS_CONDITIONS = 16;

export type StreamBypassTarget = "source_ip" | "source_region";
export type StreamBypassOperator =
  "equals" | "not_equals" | "in_cidr" | "not_in_cidr" | "in" | "not_in";

type StreamBypassPolicyContract = StreamBypassPolicy;
type StreamBypassGroupContract = StreamBypassPolicyContract["groups"][number];
type StreamBypassConditionContract =
  StreamBypassGroupContract["conditions"][number];

export type StreamBypassCondition = Omit<
  StreamBypassConditionContract,
  "operator" | "selections" | "target"
> & {
  operator: StreamBypassOperator;
  selections: GatewayVisibilitySelection[];
  target: StreamBypassTarget;
};

export type StreamBypassGroup = Omit<
  StreamBypassGroupContract,
  "conditions"
> & {
  conditions: StreamBypassCondition[];
};

export type StreamBypassPolicyForm = Omit<
  StreamBypassPolicyContract,
  "groups"
> & {
  groups: StreamBypassGroup[];
};

export const streamBypassOperators: Record<
  StreamBypassTarget,
  Array<{ labelKey: string; value: StreamBypassOperator }>
> = {
  source_ip: [
    { labelKey: "admin.advancedAuth.operatorEquals", value: "equals" },
    {
      labelKey: "admin.advancedAuth.operatorNotEquals",
      value: "not_equals",
    },
    { labelKey: "admin.advancedAuth.operatorInCidr", value: "in_cidr" },
    {
      labelKey: "admin.advancedAuth.operatorNotInCidr",
      value: "not_in_cidr",
    },
  ],
  source_region: [
    { labelKey: "admin.advancedAuth.operatorInRegion", value: "in" },
    { labelKey: "admin.advancedAuth.operatorNotInRegion", value: "not_in" },
  ],
};

const newId = (prefix: string) =>
  `${prefix}-${Math.random().toString(36).slice(2, 10)}-${Date.now().toString(36)}`;

export const createBlankStreamBypassCondition = (): StreamBypassCondition => ({
  id: newId("condition"),
  operator: "equals",
  policy_id: "",
  selections: [],
  target: "source_ip",
  values: [""],
});

export const createBlankStreamBypassGroup = (): StreamBypassGroup => ({
  conditions: [createBlankStreamBypassCondition()],
  id: newId("group"),
});

const normalizeTarget = (value: string): StreamBypassTarget =>
  value === "source_region" ? "source_region" : "source_ip";

const normalizeOperator = (
  target: StreamBypassTarget,
  value: string,
): StreamBypassOperator => {
  const allowed = streamBypassOperators[target].map((item) => item.value);
  return allowed.includes(value as StreamBypassOperator)
    ? (value as StreamBypassOperator)
    : (streamBypassOperators[target][0]?.value ?? "equals");
};

const cloneRegionSelection = (
  selection: StreamBypassConditionContract["selections"][number],
): GatewayVisibilitySelection => {
  const operator = CIDR_OPERATORS.includes(selection.operator as CidrOperator)
    ? (selection.operator as CidrOperator)
    : null;
  const normalized: GatewayVisibilitySelection = {
    city: selection.city ?? null,
    is_municipality: false,
    is_province_wide: !selection.query_city,
    label: "",
    operator,
    province: selection.province,
    query_city: selection.query_city ?? null,
    value: "",
  };
  normalized.label = getCidrRegionSelectionLabel(normalized);
  normalized.value = getCidrRegionSelectionKey(normalized);
  return normalized;
};

export const cloneStreamBypassPolicy = (
  policy: StreamBypassPolicyContract,
): StreamBypassPolicyForm => ({
  broad_rule_confirmed: false,
  enabled: policy.enabled === true,
  groups: (policy.groups ?? []).map((group) => ({
    conditions: (group.conditions ?? []).map((condition) => {
      const target = normalizeTarget(condition.target);
      return {
        ...condition,
        operator: normalizeOperator(target, condition.operator),
        selections: (condition.selections ?? []).map(cloneRegionSelection),
        target,
        values: [...(condition.values ?? [])],
      };
    }),
    id: group.id,
  })),
  policy_version: policy.policy_version ?? "",
});

export const toStreamBypassPolicyPayload = (
  form: StreamBypassPolicyForm,
  broadRuleConfirmed: boolean,
): StreamBypassPolicy => ({
  broad_rule_confirmed: broadRuleConfirmed,
  enabled: form.enabled,
  groups: form.groups.map((group) => ({
    conditions: group.conditions.map((condition) => ({
      id: condition.id,
      operator: condition.operator,
      policy_id: condition.policy_id,
      selections: condition.selections.map((selection) => ({
        city: selection.city ?? null,
        operator: selection.operator ?? null,
        province: selection.province,
        query_city: selection.query_city ?? null,
      })),
      target: condition.target,
      values: [...condition.values],
    })),
    id: group.id,
  })),
  policy_version: form.policy_version,
});

export const snapshotStreamBypassPolicy = (form: StreamBypassPolicyForm) =>
  JSON.stringify({ enabled: form.enabled, groups: form.groups });

const isBroadCidr = (value: string) => {
  const match = value.trim().match(/^.+\/(\d{1,3})$/u);
  if (!match) return false;
  const prefix = Number(match[1]);
  return Number.isInteger(prefix) && prefix <= 1;
};

export const isBroadStreamBypassPolicy = (form: StreamBypassPolicyForm) =>
  form.groups.some((group) => {
    if (group.conditions.length === 0) return false;
    if (
      group.conditions.every((condition) =>
        ["not_equals", "not_in_cidr", "not_in"].includes(condition.operator),
      )
    ) {
      return true;
    }
    return group.conditions.some(
      (condition) =>
        condition.target === "source_ip" && condition.values.some(isBroadCidr),
    );
  });

export type StreamBypassValidationIssue =
  | { kind: "empty-group" }
  | { kind: "invalid-condition" }
  | { kind: "invalid-source-address"; line: number }
  | { kind: "invalid-source-cidr"; line: number }
  | { kind: "missing-rules" };

export const getStreamBypassValidationIssue = (
  form: StreamBypassPolicyForm,
): StreamBypassValidationIssue | null => {
  if (!form.enabled) return null;
  if (form.groups.length === 0) return { kind: "missing-rules" };
  if (form.groups.some((group) => group.conditions.length === 0)) {
    return { kind: "empty-group" };
  }
  for (const condition of form.groups.flatMap((group) => group.conditions)) {
    if (condition.target === "source_region") {
      if (condition.selections.length === 0) {
        return { kind: "invalid-condition" };
      }
      continue;
    }
    if (
      condition.values.length === 0 ||
      condition.values.some((value) => !value.trim())
    ) {
      return { kind: "invalid-condition" };
    }
    const issue = getSourceNetworkValidationIssue(
      condition.values,
      condition.operator,
    );
    if (issue) {
      return {
        kind:
          issue.kind === "address"
            ? "invalid-source-address"
            : "invalid-source-cidr",
        line: issue.line,
      };
    }
  }
  return null;
};

export const createStreamBypassRuleEditor = (
  form: StreamBypassPolicyForm,
  valueDrafts: Record<string, string>,
) => {
  const sourceValue = (condition: StreamBypassCondition) =>
    formatAdvancedAuthValueList(condition.values);
  const clearDraft = (condition: StreamBypassCondition) => {
    delete valueDrafts[condition.id];
  };

  return {
    addCondition: (group: StreamBypassGroup) => {
      if (group.conditions.length < MAX_STREAM_BYPASS_CONDITIONS) {
        group.conditions.push(createBlankStreamBypassCondition());
      }
    },
    addGroup: () => {
      if (form.groups.length < MAX_STREAM_BYPASS_GROUPS) {
        form.groups.push(createBlankStreamBypassGroup());
      }
    },
    normalizeValueDraft: (condition: StreamBypassCondition) => {
      valueDrafts[condition.id] = sourceValue(condition);
    },
    operatorsFor: (target: StreamBypassTarget) => streamBypassOperators[target],
    removeCondition: (group: StreamBypassGroup, index: number) => {
      const condition = group.conditions[index];
      if (condition) clearDraft(condition);
      group.conditions.splice(index, 1);
    },
    removeGroup: (index: number) => {
      form.groups[index]?.conditions.forEach(clearDraft);
      form.groups.splice(index, 1);
    },
    setSourceValue: (condition: StreamBypassCondition, value: string) => {
      valueDrafts[condition.id] = value;
      condition.values = parseSourceNetworkTextarea(value);
    },
    sourceNetworkTranslationKey: (
      condition: StreamBypassCondition,
      suffix: "Hint" | "Label" | "Placeholder",
    ) =>
      `admin.advancedAuth.source${sourceNetworkInputKind(condition.operator) === "address" ? "Ip" : "Cidr"}${suffix}`,
    updateOperator: (
      condition: StreamBypassCondition,
      operator: StreamBypassOperator,
    ) => {
      clearDraft(condition);
      condition.operator = operator;
    },
    updateTarget: (
      condition: StreamBypassCondition,
      target: StreamBypassTarget,
    ) => {
      clearDraft(condition);
      condition.target = target;
      condition.operator = streamBypassOperators[target][0]?.value ?? "equals";
      condition.selections = [];
      condition.values = target === "source_ip" ? [""] : [];
      condition.policy_id = "";
    },
    valueInputText: (condition: StreamBypassCondition) =>
      valueDrafts[condition.id] ?? sourceValue(condition),
  };
};

export type StreamBypassRuleEditor = ReturnType<
  typeof createStreamBypassRuleEditor
>;
