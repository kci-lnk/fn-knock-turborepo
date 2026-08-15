import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import StreamBypassPolicyEditor from "../src/views/stream-mappings/StreamBypassPolicyEditor.vue";
import StreamBypassRuleGroups from "../src/views/stream-mappings/StreamBypassRuleGroups.vue";
import type { StreamBypassPolicyForm } from "../src/views/stream-mappings/stream-bypass-policy-form";
import type { StreamBypassPolicyPageModel } from "../src/views/stream-mappings/useStreamBypassPolicyPage";

const createI18nPlugin = () =>
  createI18n({
    legacy: false,
    locale: "en",
    messages: {
      en: {
        common: { cancel: "Cancel", loadingConfig: "Loading", save: "Save" },
        admin: {
          advancedAuth: {
            addAndCondition: "Add AND condition",
            addOrGroup: "Add OR group",
            addRegion: "Add region",
            deleteCondition: "Delete condition",
            deleteGroup: "Delete group",
            invalidSourceCidrLine: "Invalid CIDR {line}",
            invalidSourceIpLine: "Invalid IP {line}",
            matchOperator: "Match operator",
            matchTarget: "Match target",
            matchValue: "Match value",
            noRegions: "No regions",
            operatorEquals: "Equals",
            operatorInCidr: "In CIDR",
            operatorInRegion: "In region",
            operatorNotEquals: "Not equals",
            operatorNotInCidr: "Not in CIDR",
            operatorNotInRegion: "Not in region",
            province: "Province",
            regionDialogDescription: "Select region",
            regionLoadFailed: "Region failed",
            regionLoadFailedDescription: "Region unavailable",
            retry: "Retry",
            scope: "Scope",
            selectCity: "Select city",
            selectedRegions: "{count} selected",
            selectProvince: "Select province",
            selectProvinceFirst: "Select province first",
            sourceCidrHint: "CIDR hint",
            sourceCidrLabel: "CIDR ranges",
            sourceCidrPlaceholder: "192.0.2.0/24",
            sourceIpHint: "IP hint",
            sourceIpLabel: "IP addresses",
            sourceIpPlaceholder: "192.0.2.10",
            targetSourceIp: "Source IP",
            targetSourceRegion: "Source region",
            unavailable: "Unavailable",
          },
          streamMappings: {
            bypassPolicyDescription: "OR between groups, AND within a group.",
            policyGroupAll: "All conditions must match",
            policyAuthDisabledNotice:
              "Authentication is disabled; this remains a draft.",
            policyBroadRuleWarning: "Broad rule",
            policyDisabledSaveHint:
              "Bypass will be disabled; rules remain a draft.",
            policyEnabled: "Enable bypass",
            policyEnabledDescription: "Skip authentication for matches",
            policyNoGroups: "No groups",
            policyRegionDescription: "Compile regions to CIDRs",
            policyRuleGroups: "Source allow rules",
            policySaveHint: "Save changes",
            policyValidationNotice: "Strict validation still runs",
            savingPolicy: "Saving",
          },
        },
      },
    },
  });

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
          policy_id: "",
          selections: [],
          target: "source_ip",
          values: ["192.0.2.10"],
        },
      ],
    },
  ],
  policy_version: "version-1",
});

describe("stream bypass visual editor", () => {
  it("renders OR/AND structure and exposes only source conditions", async () => {
    const policy = form();
    const wrapper = mount(StreamBypassRuleGroups, {
      props: { form: policy, saving: false, valueDrafts: {} },
      global: {
        plugins: [createI18nPlugin()],
        stubs: { CidrRegionSelector: true },
      },
    });

    expect(wrapper.text()).toContain("Source allow rules");
    expect(wrapper.text()).toContain("OR 1");
    const target = wrapper.get("select");
    expect(
      target.findAll("option").map((option) => option.attributes("value")),
    ).toEqual(["source_ip", "source_region"]);

    const addCondition = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Add AND condition"));
    await addCondition?.trigger("click");
    expect(policy.groups[0]?.conditions).toHaveLength(2);
    expect(wrapper.text()).toContain("AND");

    const addGroup = wrapper
      .findAll("button")
      .find((button) => button.text().includes("Add OR group"));
    await addGroup?.trigger("click");
    expect(policy.groups).toHaveLength(2);
    expect(wrapper.text()).toContain("OR 2");
  });

  it("hides draft rules when login bypass cannot be enabled", () => {
    const policy = { ...form(), enabled: false, groups: [] };
    const model = {
      authEnabled: false,
      cancel: () => undefined,
      form: policy,
      isBroadRule: false,
      isDirty: false,
      save: () => undefined,
      saving: false,
      setEnabled: () => undefined,
      valueDrafts: {},
    } as unknown as StreamBypassPolicyPageModel;
    const wrapper = mount(StreamBypassPolicyEditor, {
      props: { model },
      global: {
        plugins: [createI18nPlugin()],
        stubs: {
          FloatingActionDock: {
            template: '<div><slot name="inline" /></div>',
          },
          StreamBypassRuleGroups: {
            template: '<div data-testid="rule-groups">Rule groups</div>',
          },
          Switch: {
            props: ["disabled"],
            template:
              '<button data-testid="enabled-switch" :disabled="disabled"></button>',
          },
        },
      },
    });

    expect(wrapper.text()).toContain("this remains a draft");
    expect(
      wrapper.get('[data-testid="enabled-switch"]').attributes(),
    ).toHaveProperty("disabled");
    expect(wrapper.find('[data-testid="rule-groups"]').exists()).toBe(false);
    expect(wrapper.text()).not.toContain("Strict validation still runs");
    expect(wrapper.text()).toContain("rules remain a draft");
  });
});
