import { createHash } from "node:crypto";
import { redis } from "../../redis";
import type { DDNSProviderContext } from "../types";
import {
  EDGEONE_ALLOWED_MAINLAND_REGION_CODES,
  EDGEONE_OVERSEAS_ACCESS_MODE_FIELD,
  getEdgeOneDomainTarget,
  type EdgeOneOverseasAccessMode,
  normalizeEdgeOneOverseasAccessMode,
  requestEdgeOneJson,
} from "./edgeone-shared";
import { ddnsProviderT } from "./helpers";

const EDGEONE_OVERSEAS_ACCESS_STATE_KEY_PREFIX =
  "fn_knock:ddns:edgeone:overseas_access:";
const EDGEONE_OVERSEAS_ACCESS_RULE_NAME_PREFIX = "fn_knock_block_overseas_";
const EDGEONE_OVERSEAS_ACCESS_LEGACY_RULE_NAME = "fn_knock_block_overseas";
const EDGEONE_OVERSEAS_ACCESS_SYNC_VERSION = "edgeone-overseas-console-v1";
const edgeoneOverseasT = (
  key: string,
  params?: Record<string, string | number | boolean | null | undefined>,
) => ddnsProviderT("edgeone", `overseasAccess.${key}`, params);

type EdgeOneManagedRuleScope = "zone_level_domain" | "zone_default_policy";

type EdgeOneSecurityAction = {
  Name: "Deny";
};

type EdgeOneCustomRule = {
  Action: EdgeOneSecurityAction;
  Condition: string;
  Enabled: "on" | "off";
  Id?: string;
  Name: string;
  RuleType?: "BasicAccessRule" | "PreciseMatchRule" | string;
};

type EdgeOneCustomRules = {
  Rules?: EdgeOneCustomRule[];
};

type EdgeOneSecurityPolicy = {
  CustomRules?: EdgeOneCustomRules;
};

type EdgeOneDescribeSecurityPolicyResponse = {
  SecurityPolicy?: EdgeOneSecurityPolicy;
};

type EdgeOneOverseasAccessState = {
  appliedAt: string;
  configSignature: string;
  mode: EdgeOneOverseasAccessMode;
};

export type EdgeOneOverseasAccessSyncResult = {
  changed: boolean;
  message: string | null;
};

function getStateKey(providerName: string): string {
  return `${EDGEONE_OVERSEAS_ACCESS_STATE_KEY_PREFIX}${providerName}`;
}

function getManagedRuleName(providerName: string): string {
  const suffix = createHash("sha256")
    .update(providerName)
    .digest("hex")
    .slice(0, 12);
  return `fk_eo_ovs_${suffix}`;
}

function getLegacyManagedRuleNames(providerName: string): string[] {
  return [
    `${EDGEONE_OVERSEAS_ACCESS_RULE_NAME_PREFIX}${providerName}`,
    EDGEONE_OVERSEAS_ACCESS_LEGACY_RULE_NAME,
  ];
}

function buildConfigSignature(
  providerName: string,
  target: ReturnType<typeof getEdgeOneDomainTarget>,
): string {
  return createHash("sha256")
    .update(
      [
        EDGEONE_OVERSEAS_ACCESS_SYNC_VERSION,
        providerName,
        target.zoneId,
        target.domain,
        target.endpointHost,
        target.region || "",
      ].join("\n"),
    )
    .digest("hex");
}

function safeParseState(raw: string | null): EdgeOneOverseasAccessState | null {
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<EdgeOneOverseasAccessState>;
    if (
      !parsed ||
      (parsed.mode !== "off" && parsed.mode !== "block_overseas") ||
      typeof parsed.configSignature !== "string" ||
      typeof parsed.appliedAt !== "string"
    ) {
      return null;
    }
    return {
      appliedAt: parsed.appliedAt,
      configSignature: parsed.configSignature,
      mode: parsed.mode,
    };
  } catch {
    return null;
  }
}

async function readState(
  providerName: string,
): Promise<EdgeOneOverseasAccessState | null> {
  return safeParseState(await redis.get(getStateKey(providerName)));
}

async function writeState(
  providerName: string,
  state: EdgeOneOverseasAccessState,
): Promise<void> {
  await redis.set(getStateKey(providerName), JSON.stringify(state));
}

function escapeConditionValue(value: string): string {
  return value.replace(/\\/g, "\\\\").replace(/'/g, "\\'");
}

function buildConsoleCountryCondition(): string {
  const codes = EDGEONE_ALLOWED_MAINLAND_REGION_CODES.map(
    (code) => `'${escapeConditionValue(code)}'`,
  ).join(",");
  return `not ${"${http.request.ip.country}"} in [${codes}]`;
}

function buildManagedRule(providerName: string): EdgeOneCustomRule {
  return {
    Name: getManagedRuleName(providerName),
    Condition: buildConsoleCountryCondition(),
    Action: { Name: "Deny" },
    Enabled: "on",
    RuleType: "BasicAccessRule",
  };
}

function isManagedRuleForProvider(
  providerName: string,
  rule: EdgeOneCustomRule,
): boolean {
  return (
    rule.Name === getManagedRuleName(providerName) ||
    getLegacyManagedRuleNames(providerName).includes(rule.Name)
  );
}

function isSameManagedRule(
  left: EdgeOneCustomRule | undefined,
  right: EdgeOneCustomRule,
): boolean {
  if (!left) {
    return false;
  }

  return (
    left.Name === right.Name &&
    left.Condition === right.Condition &&
    left.Enabled === right.Enabled &&
    left.RuleType === right.RuleType &&
    left.Action?.Name === right.Action.Name
  );
}

function buildAttemptLabel(options: {
  providerName: string;
  rule: EdgeOneCustomRule;
  scope: EdgeOneManagedRuleScope;
  target: ReturnType<typeof getEdgeOneDomainTarget>;
}): string {
  return [
    `provider_name=${options.providerName}`,
    `provider_target=${options.target.domain}`,
    `zone_id=${options.target.zoneId}`,
    `endpoint_host=${options.target.endpointHost}`,
    `region=${options.target.region || "empty"}`,
    `entity=${options.scope === "zone_level_domain" ? "@ZoneLevel@domain" : "ZoneDefaultPolicy"}`,
    `scope=${options.scope}`,
    `module=SecurityPolicy.CustomRules`,
    `rule_name=${options.rule.Name}`,
    `rule_type=${options.rule.RuleType || "BasicAccessRule"}`,
    `allowed_regions=${EDGEONE_ALLOWED_MAINLAND_REGION_CODES.join(",")}`,
    `condition=${options.rule.Condition}`,
  ].join(", ");
}

function getEntityPayload(
  scope: EdgeOneManagedRuleScope,
): Record<string, string> {
  return scope === "zone_level_domain"
    ? { Entity: "@ZoneLevel@domain" }
    : { Entity: "ZoneDefaultPolicy" };
}

async function describeCustomRules(
  context: DDNSProviderContext,
  target: ReturnType<typeof getEdgeOneDomainTarget>,
  scope: EdgeOneManagedRuleScope,
): Promise<EdgeOneCustomRule[]> {
  try {
    const response =
      await requestEdgeOneJson<EdgeOneDescribeSecurityPolicyResponse>(
        context,
        "DescribeSecurityPolicy",
        {
          ZoneId: target.zoneId,
          ...getEntityPayload(scope),
        },
      );

    return response.SecurityPolicy?.CustomRules?.Rules || [];
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      edgeoneOverseasT("describeRulesFailed", {
        endpointHost: target.endpointHost,
        entity:
          scope === "zone_level_domain"
            ? "@ZoneLevel@domain"
            : "ZoneDefaultPolicy",
        message,
        region: target.region || "empty",
        scope,
        target: target.domain,
        zoneId: target.zoneId,
      }),
    );
  }
}

async function modifyCustomRules(
  context: DDNSProviderContext,
  options: {
    providerName: string;
    rules: EdgeOneCustomRule[];
    scope: EdgeOneManagedRuleScope;
    target: ReturnType<typeof getEdgeOneDomainTarget>;
    trackedRule: EdgeOneCustomRule;
  },
): Promise<void> {
  try {
    await requestEdgeOneJson(context, "ModifySecurityPolicy", {
      ZoneId: options.target.zoneId,
      ...getEntityPayload(options.scope),
      SecurityPolicy: {
        CustomRules: {
          Rules: options.rules,
        },
      },
      SecurityConfig: {},
    });
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(
      edgeoneOverseasT("syncFailedWithAttempt", {
        attempt: buildAttemptLabel({
          providerName: options.providerName,
          rule: options.trackedRule,
          scope: options.scope,
          target: options.target,
        }),
        count: options.rules.length,
        message,
      }),
    );
  }
}

async function syncManagedRuleWithinScope(
  context: DDNSProviderContext,
  providerName: string,
  target: ReturnType<typeof getEdgeOneDomainTarget>,
  mode: EdgeOneOverseasAccessMode,
  scope: EdgeOneManagedRuleScope,
): Promise<boolean> {
  const existingRules = await describeCustomRules(context, target, scope);
  const managedRules = existingRules.filter((rule) =>
    isManagedRuleForProvider(providerName, rule),
  );
  const existingManagedRule = managedRules[0];
  const remainingRules = existingRules.filter(
    (rule) => !isManagedRuleForProvider(providerName, rule),
  );

  if (mode !== "block_overseas") {
    if (managedRules.length === 0) {
      return false;
    }

    await modifyCustomRules(context, {
      providerName,
      rules: remainingRules,
      scope,
      target,
      trackedRule: existingManagedRule || buildManagedRule(providerName),
    });
    return true;
  }

  const desiredRule = buildManagedRule(providerName);
  if (
    managedRules.length === 1 &&
    isSameManagedRule(existingManagedRule, desiredRule)
  ) {
    return false;
  }

  await modifyCustomRules(context, {
    providerName,
    rules: [...remainingRules, desiredRule],
    scope,
    target,
    trackedRule: desiredRule,
  });
  return true;
}

export async function ensureEdgeOneOverseasAccessSynced(options: {
  context: DDNSProviderContext;
  providerName: string;
}): Promise<EdgeOneOverseasAccessSyncResult> {
  const desiredMode = normalizeEdgeOneOverseasAccessMode(
    options.context.config[EDGEONE_OVERSEAS_ACCESS_MODE_FIELD],
  );
  const previousState = await readState(options.providerName);

  if (desiredMode === "off" && previousState?.mode !== "block_overseas") {
    return { changed: false, message: null };
  }

  const target = getEdgeOneDomainTarget(options.context.config);
  const configSignature = buildConfigSignature(options.providerName, target);

  if (
    previousState?.mode === desiredMode &&
    previousState.configSignature === configSignature
  ) {
    return { changed: false, message: null };
  }

  const scopeAttempts: EdgeOneManagedRuleScope[] = [
    "zone_level_domain",
    "zone_default_policy",
  ];
  let changed = false;
  let successCount = 0;
  const attemptErrors: string[] = [];

  for (const scope of scopeAttempts) {
    try {
      const scopeChanged = await syncManagedRuleWithinScope(
        options.context,
        options.providerName,
        target,
        desiredMode,
        scope,
      );
      changed = changed || scopeChanged;
      successCount += 1;

      if (desiredMode === "block_overseas") {
        break;
      }
    } catch (error) {
      attemptErrors.push(
        error instanceof Error ? error.message : String(error),
      );
    }
  }

  if (successCount === 0 && attemptErrors.length > 0) {
    throw new Error(
      [
        desiredMode === "block_overseas"
          ? edgeoneOverseasT("syncAllScopesFailed")
          : edgeoneOverseasT("cleanupAllScopesFailed"),
        ...attemptErrors.map((message, index) => `${index + 1}. ${message}`),
      ].join("\n"),
    );
  }

  await writeState(options.providerName, {
    appliedAt: new Date().toISOString(),
    configSignature,
    mode: desiredMode,
  });

  return {
    changed,
    message:
      desiredMode === "block_overseas"
        ? edgeoneOverseasT("syncSuccess")
        : edgeoneOverseasT("cleanupSuccess"),
  };
}
