import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { Select } from "@/components/ui/select";
import type { WAFConfig } from "../src/types/waf";
import WAFViolationRateSetting from "../src/views/system-settings/waf-settings/WAFViolationRateSetting.vue";

vi.mock("vue-i18n", () => ({ useI18n: () => ({ t: (key: string) => key }) }));
const config = (enabled = false, capacity = 5, refill = 60): WAFConfig =>
  ({
    violation_rate_limit_enabled: enabled,
    violation_rate_limit_capacity: capacity,
    violation_rate_limit_refill_seconds: refill,
  }) as WAFConfig;

describe("WAF automatic blacklist levels", () => {
  it("defaults to off and shows only a select, without submitting on mount", () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(), disabled: false },
    });
    expect(wrapper.findComponent(Select).props("modelValue")).toBe("off");
    expect(wrapper.find("input").exists()).toBe(false);
    expect(wrapper.find("form").exists()).toBe(false);
    expect(wrapper.find('button[type="submit"]').exists()).toBe(false);
    expect(wrapper.find('[role="switch"]').exists()).toBe(false);
    expect(wrapper.emitted("save")).toBeUndefined();
    wrapper.unmount();
  });
  it("saves each preset immediately and displays the server-confirmed level", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(), disabled: false },
    });
    for (const [level, capacity, refill] of [
      ["strict", 1, 120],
      ["normal", 5, 60],
      ["relaxed", 20, 10],
    ] as const) {
      wrapper.findComponent(Select).vm.$emit("update:modelValue", level);
      await wrapper.vm.$nextTick();
      expect(wrapper.emitted("save")?.at(-1)).toEqual([
        {
          violation_rate_limit_enabled: true,
          violation_rate_limit_capacity: capacity,
          violation_rate_limit_refill_seconds: refill,
        },
      ]);
      expect(wrapper.findComponent(Select).props("disabled")).toBe(true);
      await wrapper.setProps({ config: config(true, capacity, refill) });
      expect(wrapper.findComponent(Select).props("modelValue")).toBe(level);
      expect(wrapper.findComponent(Select).props("disabled")).toBe(false);
    }
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "off");
    expect(wrapper.emitted("save")?.at(-1)).toEqual([
      {
        violation_rate_limit_enabled: false,
        violation_rate_limit_capacity: 20,
        violation_rate_limit_refill_seconds: 10,
      },
    ]);
    wrapper.unmount();
  });
  it("restores the previous level after failure and permits retry", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(true, 1, 120), disabled: false },
    });
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "normal");
    await wrapper.vm.$nextTick();
    expect(wrapper.findComponent(Select).props("modelValue")).toBe("normal");
    await wrapper.setProps({ config: config(true, 1, 120) });
    expect(wrapper.findComponent(Select).props("modelValue")).toBe("strict");
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "normal");
    expect(wrapper.emitted("save")).toHaveLength(2);
    wrapper.unmount();
  });
  it("ignores unchanged, invalid, disabled and in-flight selections", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(), disabled: false },
    });
    for (const value of ["off", "custom", "invalid", undefined])
      wrapper.findComponent(Select).vm.$emit("update:modelValue", value);
    expect(wrapper.emitted("save")).toBeUndefined();
    await wrapper.setProps({ disabled: true });
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "strict");
    expect(wrapper.emitted("save")).toBeUndefined();
    await wrapper.setProps({ disabled: false });
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "strict");
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "relaxed");
    expect(wrapper.emitted("save")).toHaveLength(1);
    wrapper.unmount();
  });
  it("preserves legacy custom limits until a preset is explicitly chosen", async () => {
    const wrapper = mount(WAFViolationRateSetting, {
      props: { config: config(true, 17, 120), disabled: false },
    });
    expect(wrapper.findComponent(Select).props("modelValue")).toBe("custom");
    expect(wrapper.emitted("save")).toBeUndefined();
    await wrapper.setProps({
      config: { ...config(true, 17, 120), paranoia_level: 2 },
    });
    expect(wrapper.emitted("save")).toBeUndefined();
    wrapper.findComponent(Select).vm.$emit("update:modelValue", "normal");
    expect(wrapper.emitted("save")?.[0]).toEqual([
      {
        violation_rate_limit_enabled: true,
        violation_rate_limit_capacity: 5,
        violation_rate_limit_refill_seconds: 60,
      },
    ]);
    wrapper.unmount();
  });
});
