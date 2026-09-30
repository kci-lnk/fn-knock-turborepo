import { mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { describe, expect, it } from "vitest";
import Dialog from "../src/views/system-settings/FirewallAdditionalPortsDialog.vue";

const key = (name: string) => `admin.runModeSettings.additionalPorts.${name}`;
const mountDialog = () =>
  mount(Dialog, {
    props: {
      open: true,
      autoManageFirewallEnabled: true,
      hasUnsavedModeChanges: false,
      loadFailed: false,
      loading: false,
      modeLabel: "Direct",
      saving: false,
      details: {
        additionalPorts: [21],
        additionalRanges: [{ start: 50000, end: 51000 }],
        effectivePorts: [21, 7999],
        effectiveRanges: [{ start: 50000, end: 51000 }],
        automaticPorts: [7999],
        runType: 0,
        appliedNow: true,
      },
    },
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          missingWarn: false,
          fallbackWarn: false,
        }),
      ],
      stubs: Object.fromEntries(
        [
          "Dialog",
          "DialogContent",
          "DialogHeader",
          "DialogTitle",
          "DialogDescription",
          "DialogFooter",
          "Alert",
          "AlertTitle",
          "AlertDescription",
        ].map((name) => [name, { template: "<div><slot /></div>" }]),
      ),
    },
  });
const button = (wrapper: ReturnType<typeof mountDialog>, name: string) =>
  wrapper.findAll("button").find((item) => item.text() === key(name))!;

describe("firewall additional range editor", () => {
  it("shows saved effective ports and ranges without presenting drafts as applied", async () => {
    const wrapper = mountDialog();
    const effective = () =>
      wrapper.get(`section[aria-label="${key("effectiveTitle")}"]`);
    expect(
      effective()
        .findAll('[data-slot="badge"]')
        .map((badge) => badge.text()),
    ).toEqual(["21", "7999", "50000–51000"]);
    await wrapper
      .get(`input[aria-label="${key("rangeEndAria")}"]`)
      .setValue("52000");
    expect(effective().text()).toContain("50000–51000");
    expect(effective().text()).not.toContain("52000");
    await wrapper.setProps({
      details: {
        ...wrapper.props("details")!,
        appliedNow: false,
        runType: 1,
        automaticPorts: [],
        effectivePorts: [],
        effectiveRanges: [],
      },
    });
    expect(effective().text()).toContain(key("noPorts"));
    expect(effective().text()).not.toContain("50000");
    expect(wrapper.findAll("input")).toHaveLength(3);
    wrapper.unmount();
  });

  it("loads, edits, adds and deletes ranges and submits both kinds of entries", async () => {
    const wrapper = mountDialog();
    expect(
      button(wrapper, "saveAndApply").attributes("disabled"),
    ).toBeDefined();
    await wrapper
      .get(`input[aria-label="${key("rangeEndAria")}"]`)
      .setValue("52000");
    await button(wrapper, "saveAndApply").trigger("click");
    expect(wrapper.emitted("save")).toEqual([
      [[21], [{ start: 50000, end: 52000 }]],
    ]);
    await button(wrapper, "addRange").trigger("click");
    const starts = wrapper.findAll(
      `input[aria-label="${key("rangeStartAria")}"]`,
    );
    const ends = wrapper.findAll(`input[aria-label="${key("rangeEndAria")}"]`);
    await starts[1]!.setValue("52001");
    await ends[1]!.setValue("53000");
    expect(
      button(wrapper, "saveAndApply").attributes("disabled"),
    ).toBeUndefined();
    await wrapper
      .findAll(`button[aria-label="${key("deleteRange")}"]`)[0]!
      .trigger("click");
    await button(wrapper, "saveAndApply").trigger("click");
    expect(wrapper.emitted("save")?.[1]).toEqual([
      [21],
      [{ start: 52001, end: 53000 }],
    ]);
    wrapper.unmount();
  });
  it("blocks overlap and retains edits after saving fails", async () => {
    const wrapper = mountDialog();
    const start = wrapper.get(`input[aria-label="${key("rangeStartAria")}"]`);
    await start.setValue("21");
    expect(wrapper.get('[role="alert"]').text()).toBe(key("errors.overlap"));
    expect(
      button(wrapper, "saveAndApply").attributes("disabled"),
    ).toBeDefined();
    await start.setValue("40000");
    await wrapper.setProps({ saving: true });
    expect(start.attributes("disabled")).toBeDefined();
    await wrapper.setProps({ saving: false });
    expect((start.element as HTMLInputElement).value).toBe("40000");
    expect(
      button(wrapper, "saveAndApply").attributes("disabled"),
    ).toBeUndefined();
    wrapper.unmount();
  });
});
