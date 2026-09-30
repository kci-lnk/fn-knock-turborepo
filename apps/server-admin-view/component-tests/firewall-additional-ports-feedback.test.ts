import { mount } from "@vue/test-utils";
import { defineComponent } from "vue";
import { createI18n } from "vue-i18n";
import { afterEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import { SystemAPI } from "../src/lib/api/system";
import { useFirewallAdditionalPorts } from "../src/views/system-settings/useFirewallAdditionalPorts";

const mocks = vi.hoisted(() => ({
  success: vi.fn(),
  error: vi.fn(),
  config: { auto_manage_firewall: true },
}));
vi.mock("@/store/config", () => ({
  useConfigStore: () => ({ config: mocks.config }),
}));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { success: mocks.success, error: mocks.error },
}));

afterEach(() => {
  vi.restoreAllMocks();
  vi.clearAllMocks();
});

describe("firewall save feedback", () => {
  it.each([true, false])(
    "shows saved ports and ranges in reverse mode (auto manage: %s)",
    async (autoManage) => {
      mocks.config.auto_manage_firewall = autoManage;
      const ranges = [{ start: 50000, end: 51000 }];
      vi.spyOn(SystemAPI, "updateFirewallAdditionalPorts").mockResolvedValue({
        additionalPorts: [21],
        additionalRanges: ranges,
        automaticPorts: [],
        effectivePorts: [],
        effectiveRanges: [],
        runType: 1,
        appliedNow: false,
      });
      const wrapper = mount(
        defineComponent({
          setup: () =>
            useFirewallAdditionalPorts({
              canManageHostFirewall: () => true,
              hasUnsavedModeChanges: () => false,
            }),
          template: "<div />",
        }),
        {
          global: {
            plugins: [
              createI18n({
                legacy: false,
                locale: "en",
                messages: { en: { admin: enAdmin } },
              }),
            ],
          },
        },
      );
      await wrapper.vm.save([21], ranges);
      expect(SystemAPI.updateFirewallAdditionalPorts).toHaveBeenCalledWith(
        [21],
        ranges,
      );
      expect(mocks.error).not.toHaveBeenCalled();
      expect(mocks.success).toHaveBeenCalledWith(expect.any(String), {
        description: expect.stringContaining(
          "Saved ports and ranges: 21, 50000–51000.",
        ),
      });
      wrapper.unmount();
    },
  );
});
