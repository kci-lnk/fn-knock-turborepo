import assert from "node:assert/strict";
import { describe, it } from "node:test";
import {
  cloneStreamBypassPolicy,
  createStreamBypassRuleEditor,
  getStreamBypassValidationIssue,
  isBroadStreamBypassPolicy,
  snapshotStreamBypassPolicy,
  toStreamBypassPolicyPayload,
  type StreamBypassPolicyForm,
} from "../src/views/stream-mappings/stream-bypass-policy-form";

const form = (): StreamBypassPolicyForm => ({
  broad_rule_confirmed: false,
  enabled: true,
  groups: [
    {
      id: "group-1",
      conditions: [
        {
          id: "condition-1",
          operator: "equals",
          policy_id: "compiled-old",
          selections: [],
          target: "source_ip",
          values: ["192.0.2.10"],
        },
      ],
    },
  ],
  policy_version: "version-1",
});

describe("stream bypass visual form model", () => {
  it("normalizes API data without retaining broad-rule acknowledgement", () => {
    const cloned = cloneStreamBypassPolicy({
      broad_rule_confirmed: true,
      enabled: true,
      groups: [
        {
          id: "group-1",
          conditions: [
            {
              id: "condition-1",
              operator: "in",
              policy_id: "compiled-1",
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
              target: "source_region",
              values: [],
            },
          ],
        },
      ],
      policy_version: "version-1",
    });

    assert.equal(cloned.broad_rule_confirmed, false);
    assert.equal(
      cloned.groups[0]?.conditions[0]?.selections[0]?.label,
      "深圳 · 电信",
    );
    assert.equal(
      cloned.groups[0]?.conditions[0]?.selections[0]?.value,
      "广东::深圳::电信",
    );
  });

  it("validates enabled, empty, malformed IP, and region conditions", () => {
    assert.deepEqual(
      getStreamBypassValidationIssue({ ...form(), groups: [] }),
      { kind: "missing-rules" },
    );
    assert.deepEqual(
      getStreamBypassValidationIssue({
        ...form(),
        groups: [{ id: "empty", conditions: [] }],
      }),
      { kind: "empty-group" },
    );
    const badAddress = form();
    badAddress.groups[0]!.conditions[0]!.values = ["192.0.2.0/24"];
    assert.deepEqual(getStreamBypassValidationIssue(badAddress), {
      kind: "invalid-source-address",
      line: 1,
    });
    const missingRegion = form();
    Object.assign(missingRegion.groups[0]!.conditions[0]!, {
      operator: "in",
      selections: [],
      target: "source_region",
      values: [],
    });
    assert.deepEqual(getStreamBypassValidationIssue(missingRegion), {
      kind: "invalid-condition",
    });
    assert.equal(getStreamBypassValidationIssue(form()), null);
  });

  it("does not validate hidden rule drafts while bypass is disabled", () => {
    const disabled = form();
    disabled.enabled = false;
    disabled.groups = [{ id: "unfinished", conditions: [] }];
    assert.equal(getStreamBypassValidationIssue(disabled), null);
  });

  it("detects negative-only and near-global rules", () => {
    const negative = form();
    negative.groups[0]!.conditions[0]!.operator = "not_equals";
    assert.equal(isBroadStreamBypassPolicy(negative), true);

    const halfInternet = form();
    Object.assign(halfInternet.groups[0]!.conditions[0]!, {
      operator: "in_cidr",
      values: ["0.0.0.0/1"],
    });
    assert.equal(isBroadStreamBypassPolicy(halfInternet), true);
    assert.equal(isBroadStreamBypassPolicy(form()), false);
  });

  it("keeps mutation and source value parsing inside the editor model", () => {
    const current = form();
    const drafts: Record<string, string> = {};
    const editor = createStreamBypassRuleEditor(current, drafts);
    const condition = current.groups[0]!.conditions[0]!;

    editor.setSourceValue(condition, "192.0.2.10\n192.0.2.10\n2001:db8::10");
    assert.deepEqual(condition.values, ["192.0.2.10", "2001:db8::10"]);
    editor.updateTarget(condition, "source_region");
    assert.equal(condition.operator, "in");
    assert.equal(drafts[condition.id], undefined);
    editor.addCondition(current.groups[0]!);
    editor.addGroup();
    assert.equal(current.groups.length, 2);
    assert.equal(current.groups[0]!.conditions.length, 2);
  });

  it("keeps CAS metadata out of dirty snapshots and acknowledgement explicit", () => {
    const current = form();
    const changedVersion = { ...current, policy_version: "version-2" };
    assert.equal(
      snapshotStreamBypassPolicy(current),
      snapshotStreamBypassPolicy(changedVersion),
    );
    const payload = toStreamBypassPolicyPayload(current, true);
    assert.equal(payload.policy_version, "version-1");
    assert.equal(payload.broad_rule_confirmed, true);
  });
});
