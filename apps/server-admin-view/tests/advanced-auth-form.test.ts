import assert from "node:assert/strict";
import { describe, it } from "node:test";
import type { AdvancedAuthCondition, AdvancedAuthConfig } from "../src/types";
import {
  advancedAuthHourInputToSeconds,
  cloneAdvancedAuthConfig,
  createAdvancedAuthRuleEditor,
  getAdvancedAuthValidationIssue,
  isAdvancedAuthBroadRule,
  MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
  MIN_ADVANCED_AUTH_TTL_SECONDS,
  secondsToAdvancedAuthHourInput,
  snapshotAdvancedAuthConfig,
} from "../src/views/subdomain-proxy/advanced-auth-form";

const condition = (
  overrides: Partial<AdvancedAuthCondition> = {},
): AdvancedAuthCondition => ({
  id: "condition-1",
  target: "source_ip",
  operator: "equals",
  name: "",
  values: ["192.0.2.1"],
  selections: [],
  ...overrides,
});

const config = (
  conditionOverrides: Partial<AdvancedAuthCondition> = {},
): AdvancedAuthConfig => ({
  enabled: true,
  idle_ttl_seconds: 3_600,
  max_lifetime_seconds: 7_200,
  groups: [
    {
      id: "group-1",
      conditions: [condition(conditionOverrides)],
    },
  ],
});

describe("advanced authentication form model", () => {
  it("normalizes compiled source addresses and region labels at the API boundary", () => {
    const cloned = cloneAdvancedAuthConfig({
      enabled: true,
      idle_ttl_seconds: 0,
      max_lifetime_seconds: 0,
      groups: [
        {
          id: "group-1",
          conditions: [
            {
              id: "condition-1",
              target: "source_ip",
              operator: "equals",
              name: "",
              values: [],
              cidrs: ["192.0.2.1/32", "2001:db8::1/128"],
              selections: [
                {
                  province: "广东",
                  city: "深圳",
                  query_city: "深圳",
                  operator: "电信",
                  label: "深圳",
                  cidrs: [],
                },
              ],
            },
          ],
        },
      ],
    });

    assert.deepEqual(cloned.groups[0]?.conditions[0]?.values, [
      "192.0.2.1",
      "2001:db8::1",
    ]);
    assert.equal(
      cloned.groups[0]?.conditions[0]?.selections[0]?.label,
      "深圳 · 电信",
    );
    assert.equal(cloned.idle_ttl_seconds, 86_400);
    assert.equal(cloned.max_lifetime_seconds, 2_592_000);
  });

  it("reports validation failures in the same user-facing priority", () => {
    assert.deepEqual(
      getAdvancedAuthValidationIssue({ ...config(), groups: [] }),
      { kind: "invalid-rules" },
    );
    assert.deepEqual(
      getAdvancedAuthValidationIssue({
        ...config(),
        groups: [{ id: "group-1", conditions: [] }],
      }),
      { kind: "empty-group" },
    );
    assert.deepEqual(
      getAdvancedAuthValidationIssue(config({ values: ["192.0.2.0/24"] })),
      { kind: "invalid-source-address", line: 1 },
    );
    assert.deepEqual(
      getAdvancedAuthValidationIssue(
        config({ target: "request_header", name: "", values: ["yes"] }),
      ),
      { kind: "invalid-condition" },
    );
    assert.deepEqual(
      getAdvancedAuthValidationIssue({
        ...config(),
        enabled: false,
        idle_ttl_seconds: 7_200,
        max_lifetime_seconds: 3_600,
      }),
      { kind: "max-lifetime-too-short" },
    );
    assert.equal(getAdvancedAuthValidationIssue(config()), null);
  });

  it("detects each broad-rule shape that requires acknowledgement", () => {
    assert.equal(
      isAdvancedAuthBroadRule(config({ operator: "not_equals" })),
      true,
    );
    assert.equal(
      isAdvancedAuthBroadRule(
        config({ target: "http_method", operator: "in", values: ["GET"] }),
      ),
      true,
    );
    assert.equal(
      isAdvancedAuthBroadRule(
        config({ target: "url_path", operator: "prefix", values: ["/"] }),
      ),
      true,
    );
    assert.equal(
      isAdvancedAuthBroadRule(
        config({ operator: "in_cidr", values: ["0.0.0.0/0"] }),
      ),
      true,
    );
    assert.equal(isAdvancedAuthBroadRule(config()), false);
  });

  it("clamps hour inputs to the API TTL boundaries", () => {
    assert.equal(secondsToAdvancedAuthHourInput(300), 0.08);
    assert.equal(advancedAuthHourInputToSeconds(0, 10_000), 300);
    assert.equal(
      advancedAuthHourInputToSeconds(100_000, 10_000),
      10_000,
    );
    assert.equal(
      advancedAuthHourInputToSeconds(Number.NaN, 10_000),
      MIN_ADVANCED_AUTH_TTL_SECONDS,
    );
    assert.equal(
      advancedAuthHourInputToSeconds(
        MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS / 3_600,
        MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
      ),
      MAX_ADVANCED_AUTH_IDLE_TTL_SECONDS,
    );
  });

  it("excludes derived policy metadata from dirty-state snapshots", () => {
    const baseline = config();
    assert.equal(
      snapshotAdvancedAuthConfig({ ...baseline, policy_version: 1 }),
      snapshotAdvancedAuthConfig({ ...baseline, policy_version: 2 }),
    );
  });

  it("keeps condition drafts and group mutations inside the editor model", () => {
    const form = config();
    const drafts: Record<string, string> = {};
    const editor = createAdvancedAuthRuleEditor(form, drafts);
    const first = form.groups[0]!.conditions[0]!;

    editor.setSourceIpValue(first, "192.0.2.1\n192.0.2.1\n2001:db8::1");
    assert.deepEqual(first.values, ["192.0.2.1", "2001:db8::1"]);
    assert.equal(editor.valueInputText(first), "192.0.2.1\n192.0.2.1\n2001:db8::1");

    editor.updateTarget(first, "request_header");
    assert.equal(drafts[first.id], undefined);
    assert.equal(first.operator, "exists");
    assert.deepEqual(first.values, [""]);
    editor.updateOperator(first, "not_exists");
    assert.deepEqual(first.values, []);

    editor.addCondition(form.groups[0]!);
    assert.equal(form.groups[0]!.conditions.length, 2);
    editor.addGroup();
    assert.equal(form.groups.length, 2);
    editor.removeGroup(0);
    assert.equal(form.groups.length, 1);
  });
});
