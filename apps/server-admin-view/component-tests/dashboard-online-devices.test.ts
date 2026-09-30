import { mount } from "@vue/test-utils";
import { describe, expect, it, vi } from "vitest";
import { createFnKnockI18n } from "@fn-knock/i18n/vue/admin";
import OnlineDeviceIcons from "../src/views/dashboard/OnlineDeviceIcons.vue";

const render = async (devices?: { type: string; count: number }[]) => {
  const i18n = await createFnKnockI18n({
    scope: "admin",
    defaultLocale: "zh-CN",
  });
  return mount(OnlineDeviceIcons, {
    attachTo: document.body,
    props: { devices },
    global: { plugins: [i18n] },
  });
};

describe("online identity device icons", () => {
  it("shows one icon per platform and only marks counts above one", async () => {
    const wrapper = await render([
      { type: "windows", count: 2 },
      { type: "iphone", count: 1 },
      { type: "macos", count: 1 },
    ]);
    expect(wrapper.findAll('[role="img"]')).toHaveLength(3);
    expect(wrapper.text()).toBe("×2");
    const windows = wrapper.get('[data-device="windows"]');
    expect(windows.attributes("aria-label")).toBe("Windows：2 个在线身份");
    expect(
      wrapper
        .findAll('[role="img"]')
        .map((entry) => entry.attributes("data-device")),
    ).toEqual(["macos", "windows", "iphone"]);
    wrapper.unmount();
  });
  it("shows the platform explanation on keyboard focus", async () => {
    const wrapper = await render([{ type: "windows", count: 2 }]);
    try {
      const trigger = wrapper.get<HTMLElement>('[data-device="windows"]');
      expect(trigger.attributes("tabindex")).toBe("0");
      trigger.element.focus();
      await vi.waitFor(() => {
        expect(
          document.querySelector('[role="tooltip"]')?.textContent,
        ).toContain("Windows：2 个在线身份");
      });
      await trigger.trigger("keydown", { key: "Escape" });
      await vi.waitFor(() =>
        expect(document.querySelector('[role="tooltip"]')).toBeNull(),
      );
    } finally {
      wrapper.unmount();
    }
  });
  it("supports all platforms and groups future types into unknown", async () => {
    const wrapper = await render([
      ...[
        "macos",
        "windows",
        "iphone",
        "ipad",
        "android",
        "linux",
        "chromeos",
        "unknown",
      ].map((type) => ({ type, count: 1 })),
      { type: "future-os", count: 1 },
      { type: "windows", count: 0 },
    ]);
    expect(wrapper.findAll("svg")).toHaveLength(8);
    expect(
      wrapper.get('[data-device="unknown"]').attributes("aria-label"),
    ).toBe("未知设备：2 个在线身份");
    wrapper.unmount();
  });
  it("omits the group for legacy responses and updates after refresh", async () => {
    const wrapper = await render();
    expect(wrapper.find('[data-testid="online-device-icons"]').exists()).toBe(
      false,
    );
    await wrapper.setProps({ devices: [{ type: "iphone", count: 1 }] });
    expect(wrapper.find('[data-device="iphone"]').exists()).toBe(true);
    await wrapper.setProps({ devices: [] });
    expect(wrapper.find('[data-testid="online-device-icons"]').exists()).toBe(
      false,
    );
    wrapper.unmount();
  });
});
