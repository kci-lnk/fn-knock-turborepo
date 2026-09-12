import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import type { WAFConfig } from "../src/types/waf";
import WAFViolationRateSetting from "../src/views/system-settings/waf-settings/WAFViolationRateSetting.vue";
import WAFSettingSwitchRow from "../src/views/system-settings/waf-settings/WAFSettingSwitchRow.vue";
vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
const config = (enabled = false): WAFConfig =>
  ({
    violation_rate_limit_enabled: enabled,
    violation_rate_limit_capacity: 5,
    violation_rate_limit_refill_seconds: 60,
  }) as WAFConfig;
describe("WAF violation frequency settings", () => {
  it("starts disabled, edits locally and submits all values together", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(), disabled: false },
    });
    expect(wrapper.findAll('input[type="number"]')).toHaveLength(0);
    wrapper.findComponent(WAFSettingSwitchRow).vm.$emit("change", true);
    await wrapper.vm.$nextTick();
    const inputs = wrapper.findAll('input[type="number"]');
    expect(inputs).toHaveLength(2);
    await inputs[0]!.setValue(10);
    await inputs[1]!.setValue(120);
    expect(wrapper.emitted("save")).toBeUndefined();
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("save")?.[0]).toEqual([
      {
        violation_rate_limit_enabled: true,
        violation_rate_limit_capacity: 10,
        violation_rate_limit_refill_seconds: 120,
      },
    ]);
    wrapper.unmount();
  });
  it("rejects invalid inputs and restores server values after a failed save", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(true), disabled: false },
    });
    const inputs = wrapper.findAll('input[type="number"]');
    for (const value of [0, 1.5, 10001]) {
      await inputs[0]!.setValue(value);
      await wrapper.find("form").trigger("submit");
    }
    expect(wrapper.emitted("save")).toBeUndefined();
    // A submitted, valid request can fail and return the same server values.
    await inputs[0]!.setValue(10);
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("save")).toHaveLength(1);
    await wrapper.setProps({ config: config(true) });
    expect((inputs[0]!.element as HTMLInputElement).value).toBe("5");
    await wrapper.setProps({ disabled: true });
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("save")).toHaveLength(1);
    wrapper.unmount();
  });
});

describe("WAF frequency draft edge cases", () => {
  it("can disable protection with an invalid draft using the last saved parameters", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(true), disabled: false },
    });
    await wrapper.find('input[type="number"]').setValue(0);
    wrapper.findComponent(WAFSettingSwitchRow).vm.$emit("change", false);
    await wrapper.vm.$nextTick();
    expect(
      wrapper.find('button[type="submit"]').attributes("disabled"),
    ).toBeUndefined();
    await wrapper.find("form").trigger("submit");
    expect(wrapper.emitted("save")?.[0]).toEqual([
      {
        violation_rate_limit_enabled: false,
        violation_rate_limit_capacity: 5,
        violation_rate_limit_refill_seconds: 60,
      },
    ]);
    wrapper.unmount();
  });
  it("retains drafts across unrelated WAF refreshes but adopts changed limits", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(true), disabled: false },
    });
    await wrapper.find('input[type="number"]').setValue(10);
    await wrapper.setProps({ config: { ...config(true), paranoia_level: 2 } });
    expect(
      (wrapper.find('input[type="number"]').element as HTMLInputElement).value,
    ).toBe("10");
    await wrapper.setProps({
      config: { ...config(true), violation_rate_limit_capacity: 20 },
    });
    expect(
      (wrapper.find('input[type="number"]').element as HTMLInputElement).value,
    ).toBe("20");
    wrapper.unmount();
  });
});

it("ignores switch label clicks while a save is in progress", async () => {
  const wrapper = mount(WAFViolationRateSetting, {
    props: { config: config(), disabled: true },
  });
  await wrapper
    .findComponent(WAFSettingSwitchRow)
    .find("label")
    .trigger("click");
  expect(wrapper.findAll('input[type="number"]')).toHaveLength(0);
  wrapper.unmount();
});
