import type {
  AdvancedAuthCondition,
  AdvancedAuthConditionContract,
  AdvancedAuthConditionTarget,
  AdvancedAuthConfig,
  AdvancedAuthConfigContract,
  AdvancedAuthOperator,
  AdvancedAuthRuleGroup,
} from "../../types";
import { getCidrRegionSelectionLabel } from "../../types/cidr";
import {
  formatAdvancedAuthValueList,
  getSourceNetworkValidationIssue,
  parseAdvancedAuthValueList,
  parseSourceNetworkTextarea,
  sourceNetworkInputKind,
} from "./advanced-auth-source-network";

export const MAX_ADVANCED_AUTH_GROUPS = 16;
export const MAX_ADVANCED_AUTH_CONDITIONS = 16;
export const SECONDS_PER_MINUTE = 60;
export const SECONDS_PER_HOUR = 60 * SECONDS_PER_MINUTE;
export const MIN_ADVANCED_AUTH_TTL_SECONDS = 5 * SECONDS_PER_MINUTE;
export const MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS =
  30 * 24 * SECONDS_PER_HOUR;
export const MAX_ADVANCED_AUTH_LIFETIME_SECONDS =
  365 * 24 * SECONDS_PER_HOUR;
export const MIN_ADVANCED_AUTH_TTL_HOURS = Number(
  (MIN_ADVANCED_AUTH_TTL_SECONDS / SECONDS_PER_HOUR).toFixed(2),
);
export const MAX_ADVANCED_AUTH_IDLE_TTL_HOURS =
  MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS / SECONDS_PER_HOUR;
export const MAX_ADVANCED_AUTH_LIFETIME_HOURS =
  MAX_ADVANCED_AUTH_LIFETIME_SECONDS / SECONDS_PER_HOUR;

export const advancedAuthTargetOptions: Array<{
  value: AdvancedAuthConditionTarget;
  labelKey: string;
}> = [
  { value: "source_ip", labelKey: "admin.advancedAuth.targetSourceIp" },
  { value: "source_region", labelKey: "admin.advancedAuth.targetSourceRegion" },
  { value: "url_path", labelKey: "admin.advancedAuth.targetUrlPath" },
  {
    value: "request_header",
    labelKey: "admin.advancedAuth.targetRequestHeader",
  },
  {
    value: "query_parameter",
    labelKey: "admin.advancedAuth.targetQueryParameter",
  },
  { value: "http_method", labelKey: "admin.advancedAuth.targetHttpMethod" },
];

const commonNamedValueOperators: Array<{
  value: AdvancedAuthOperator;
  labelKey: string;
}> = [
  { value: "exists", labelKey: "admin.advancedAuth.operatorExists" },
  { value: "not_exists", labelKey: "admin.advancedAuth.operatorNotExists" },
  { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
  { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
  { value: "contains", labelKey: "admin.advancedAuth.operatorContains" },
  {
    value: "not_contains",
    labelKey: "admin.advancedAuth.operatorNotContains",
  },
  { value: "starts_with", labelKey: "admin.advancedAuth.operatorStartsWith" },
  {
    value: "not_starts_with",
    labelKey: "admin.advancedAuth.operatorNotStartsWith",
  },
  { value: "ends_with", labelKey: "admin.advancedAuth.operatorEndsWith" },
  {
    value: "not_ends_with",
    labelKey: "admin.advancedAuth.operatorNotEndsWith",
  },
  { value: "regex", labelKey: "admin.advancedAuth.operatorRegex" },
  { value: "not_regex", labelKey: "admin.advancedAuth.operatorNotRegex" },
];

export const advancedAuthOperatorsByTarget: Record<
  AdvancedAuthConditionTarget,
  Array<{ value: AdvancedAuthOperator; labelKey: string }>
> = {
  source_ip: [
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "in_cidr", labelKey: "admin.advancedAuth.operatorInCidr" },
    { value: "not_in_cidr", labelKey: "admin.advancedAuth.operatorNotInCidr" },
  ],
  source_region: [
    { value: "in", labelKey: "admin.advancedAuth.operatorInRegion" },
    { value: "not_in", labelKey: "admin.advancedAuth.operatorNotInRegion" },
  ],
  url_path: [
    { value: "equals", labelKey: "admin.advancedAuth.operatorEquals" },
    { value: "not_equals", labelKey: "admin.advancedAuth.operatorNotEquals" },
    { value: "prefix", labelKey: "admin.advancedAuth.operatorPrefix" },
    { value: "not_prefix", labelKey: "admin.advancedAuth.operatorNotPrefix" },
    { value: "contains", labelKey: "admin.advancedAuth.operatorContains" },
    {
      value: "not_contains",
      labelKey: "admin.advancedAuth.operatorNotContains",
    },
    { value: "regex", labelKey: "admin.advancedAuth.operatorRegex" },
    { value: "not_regex", labelKey: "admin.advancedAuth.operatorNotRegex" },
  ],
  request_header: [...commonNamedValueOperators],
  query_parameter: [...commonNamedValueOperators],
  http_method: [
    { value: "in", labelKey: "admin.advancedAuth.operatorMethodIn" },
    { value: "not_in", labelKey: "admin.advancedAuth.operatorMethodNotIn" },
  ],
};

const newId = (prefix: string) =>
  `${prefix}-${Math.random().toString(36).slice(2, 10)}-${Date.now().toString(36)}`;

export const createBlankAdvancedAuthCondition = (): AdvancedAuthCondition => ({
  id: newId("condition"),
  target: "source_ip",
  operator: "equals",
  name: "",
  values: [""],
  selections: [],
});

export const createBlankAdvancedAuthGroup = (): AdvancedAuthRuleGroup => ({
  id: newId("group"),
  conditions: [createBlankAdvancedAuthCondition()],
});

export const cloneAdvancedAuthCondition = (
  condition: AdvancedAuthConditionContract | AdvancedAuthCondition,
): AdvancedAuthCondition => {
  const compiledValues = condition.cidrs ?? [];
  const values = condition.values?.length
    ? [...condition.values]
    : condition.target === "source_ip"
      ? compiledValues.map((value) =>
          condition.operator === "equals" || condition.operator === "not_equals"
            ? value.replace(/\/(32|128)$/, "")
            : value,
        )
      : [];
  return {
    ...condition,
    values,
    selections: (condition.selections ?? []).map((selection) => ({
      ...selection,
      label: getCidrRegionSelectionLabel(selection),
    })),
    cidrs: [...compiledValues],
  };
};

export const cloneAdvancedAuthConfig = (
  config: AdvancedAuthConfigContract | AdvancedAuthConfig,
): AdvancedAuthConfig => ({
  enabled: config.enabled === true,
  idle_ttl_seconds: Number(config.idle_ttl_seconds) || 24 * SECONDS_PER_HOUR,
  max_lifetime_seconds:
    Number(config.max_lifetime_seconds) || 30 * 24 * SECONDS_PER_HOUR,
  policy_version: config.policy_version,
  groups: (config.groups ?? []).map((group) => ({
    id: group.id,
    conditions: (group.conditions ?? []).map(cloneAdvancedAuthCondition),
  })),
});

export const getAdvancedAuthSourceIpDisplayValue = (
  condition: AdvancedAuthCondition,
) => {
  const values = condition.values?.length
    ? condition.values
    : (condition.cidrs ?? []);
  return formatAdvancedAuthValueList(
    values.map((value) => {
      if (
        condition.operator === "equals" ||
        condition.operator === "not_equals"
      ) {
        return value.replace(/\/(32|128)$/, "");
      }
      return value;
    }),
  );
};

export const getSourceNetworkTranslationKey = (
  condition: AdvancedAuthCondition,
  suffix: "Label" | "Placeholder" | "Hint",
) =>
  `admin.advancedAuth.source${sourceNetworkInputKind(condition.operator) === "address" ? "Ip" : "Cidr"}${suffix}`;

export const advancedAuthConditionNeedsValue = (
  condition: AdvancedAuthCondition,
) =>
  condition.target !== "source_region" &&
  condition.operator !== "exists" &&
  condition.operator !== "not_exists";

export const createAdvancedAuthRuleEditor = (
  form: AdvancedAuthConfig,
  valueDrafts: Record<string, string>,
) => {
  const sourceIpDisplayValue = getAdvancedAuthSourceIpDisplayValue;
  const clearValueDraft = (condition: AdvancedAuthCondition) => {
    delete valueDrafts[condition.id];
  };
  const valueText = (condition: AdvancedAuthCondition) =>
    formatAdvancedAuthValueList(condition.values ?? []);

  return {
    sourceIpDisplayValue,
    sourceNetworkTranslationKey: getSourceNetworkTranslationKey,
    needsValue: advancedAuthConditionNeedsValue,
    operatorsFor: (target: AdvancedAuthConditionTarget) =>
      advancedAuthOperatorsByTarget[target],
    setSourceIpValue: (condition: AdvancedAuthCondition, value: string) => {
      valueDrafts[condition.id] = value;
      condition.values = parseSourceNetworkTextarea(value);
    },
    valueText,
    setValueText: (condition: AdvancedAuthCondition, value: string) => {
      valueDrafts[condition.id] = value;
      condition.values = parseAdvancedAuthValueList(value);
    },
    valueInputText: (condition: AdvancedAuthCondition) =>
      valueDrafts[condition.id] ??
      (condition.target === "source_ip"
        ? sourceIpDisplayValue(condition)
        : valueText(condition)),
    normalizeValueDraft: (condition: AdvancedAuthCondition) => {
      valueDrafts[condition.id] =
        condition.target === "source_ip"
          ? sourceIpDisplayValue(condition)
          : valueText(condition);
    },
    updateTarget: (
      condition: AdvancedAuthCondition,
      target: AdvancedAuthConditionTarget,
    ) => {
      clearValueDraft(condition);
      condition.target = target;
      condition.operator =
        advancedAuthOperatorsByTarget[target][0]?.value ?? "equals";
      condition.values = target === "source_region" ? [] : [""];
      condition.selections = [];
      condition.cidrs = undefined;
    },
    updateOperator: (
      condition: AdvancedAuthCondition,
      operator: AdvancedAuthOperator,
    ) => {
      clearValueDraft(condition);
      condition.operator = operator;
      if (operator === "exists" || operator === "not_exists") {
        condition.values = [];
      }
    },
    addGroup: () => {
      if (form.groups.length >= MAX_ADVANCED_AUTH_GROUPS) return;
      form.groups.push(createBlankAdvancedAuthGroup());
    },
    removeGroup: (groupIndex: number) => {
      form.groups[groupIndex]?.conditions.forEach(clearValueDraft);
      form.groups.splice(groupIndex, 1);
    },
    addCondition: (group: AdvancedAuthRuleGroup) => {
      if (group.conditions.length >= MAX_ADVANCED_AUTH_CONDITIONS) return;
      group.conditions.push(createBlankAdvancedAuthCondition());
    },
    removeCondition: (group: AdvancedAuthRuleGroup, index: number) => {
      const condition = group.conditions[index];
      if (condition) clearValueDraft(condition);
      group.conditions.splice(index, 1);
    },
  };
};

export const secondsToAdvancedAuthHourInput = (seconds: number) => {
  const hours = seconds / SECONDS_PER_HOUR;
  return Number.isInteger(hours) ? hours : Number(hours.toFixed(2));
};

export const advancedAuthHourInputToSeconds = (
  value: number,
  maximum: number,
) => {
  const hours = Number(value);
  if (!Number.isFinite(hours)) return MIN_ADVANCED_AUTH_TTL_SECONDS;
  return Math.min(
    maximum,
    Math.max(
      MIN_ADVANCED_AUTH_TTL_SECONDS,
      Math.round(hours * 60) * SECONDS_PER_MINUTE,
    ),
  );
};

export const snapshotAdvancedAuthConfig = (config: AdvancedAuthConfig) =>
  JSON.stringify({
    enabled: config.enabled,
    idle_ttl_seconds: config.idle_ttl_seconds,
    max_lifetime_seconds: config.max_lifetime_seconds,
    groups: config.groups,
  });

export const isAdvancedAuthBroadRule = (config: AdvancedAuthConfig) =>
  config.groups.some((group) => {
    const conditions = group.conditions;
    if (!conditions.length) return false;
    if (conditions.every((condition) => condition.operator.startsWith("not_")))
      return true;
    if (conditions.length === 1 && conditions[0]?.target === "http_method")
      return true;
    return conditions.some(
      (condition) =>
        (condition.target === "url_path" &&
          (condition.operator === "prefix" ||
            condition.operator === "not_prefix") &&
          (condition.values ?? []).includes("/")) ||
        (condition.target === "source_ip" &&
          (condition.values ?? []).some((value) =>
            ["0.0.0.0/0", "::/0"].includes(value.trim()),
          )),
    );
  });

export type AdvancedAuthValidationIssue =
  | { kind: "invalid-rules" }
  | { kind: "empty-group" }
  | { kind: "invalid-source-address"; line: number }
  | { kind: "invalid-source-cidr"; line: number }
  | { kind: "invalid-condition" }
  | { kind: "max-lifetime-too-short" };

export const getAdvancedAuthValidationIssue = (
  config: AdvancedAuthConfig,
): AdvancedAuthValidationIssue | null => {
  if (config.enabled) {
    if (config.groups.length === 0) return { kind: "invalid-rules" };
    if (config.groups.some((group) => group.conditions.length === 0)) {
      return { kind: "empty-group" };
    }
    const conditions = config.groups.flatMap((group) => group.conditions);
    const invalidSourceNetwork = conditions
      .filter((condition) => condition.target === "source_ip")
      .map((condition) =>
        getSourceNetworkValidationIssue(
          condition.values ?? [],
          condition.operator,
        ),
      )
      .find((issue) => issue != null);
    if (invalidSourceNetwork) {
      return {
        kind:
          invalidSourceNetwork.kind === "address"
            ? "invalid-source-address"
            : "invalid-source-cidr",
        line: invalidSourceNetwork.line,
      };
    }
    const invalidCondition = conditions.some(
      (condition) =>
        (condition.target === "source_region" &&
          (condition.selections ?? []).length === 0) ||
        ((condition.target === "request_header" ||
          condition.target === "query_parameter") &&
          !condition.name?.trim()) ||
        (advancedAuthConditionNeedsValue(condition) &&
          ((condition.values ?? []).length === 0 ||
            (condition.values ?? []).some((value) => !value.trim()))),
    );
    if (invalidCondition) return { kind: "invalid-condition" };
  }
  if (config.max_lifetime_seconds < config.idle_ttl_seconds) {
    return { kind: "max-lifetime-too-short" };
  }
  return null;
};
