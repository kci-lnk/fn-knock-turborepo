import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { createMemoryHistory, createRouter } from "vue-router";
import { describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";
import SystemSettings from "../src/views/SystemSettings.vue";

vi.mock("../src/store/config", () => ({
  useConfigStore: () => ({
    config: { run_type: 1 },
    canUseFrpc: false,
    canUseCloudflared: false,
    canUseAcme: false,
    isLinuxDeployment: true,
    isProtectedAdminPanelDeployment: false,
  }),
}));

vi.mock("../src/views/system-settings/RunModeSettings.vue", () => ({
  default: { template: "<div>Run mode settings</div>" },
}));
vi.mock("../src/views/AboutUpdate.vue", () => ({
  default: { template: "<div>Update settings</div>" },
}));

const settings = [
  "RunModeSettings", "FrpSettings", "CloudflaredSettings", "AcmeSSL",
  "IpLocationSettings", "ScannerFirewallSettings", "FeaturesSettings",
  "FnosSettings", "CaptchaSettings", "GatewayLoggingSettings", "GatewaySettings",
  "WAFSettings", "SessionSettings", "MaintenanceSettings", "PanelSettings",
  "AboutUpdate",
];

async function setup(path: string) {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/system", component: SystemSettings }],
  });
  await router.push(path);
  await router.isReady();
  const wrapper = mount(SystemSettings, {
    global: {
      plugins: [router, createI18n({
        legacy: false,
        locale: "en",
        messages: { en: { admin: enAdmin } },
      })],
      stubs: Object.fromEntries(settings.map((name) => [name, true])),
    },
  });
  await flushPromises();
  return { wrapper, router };
}

describe("system update settings tab", () => {
  it("opens a direct update link and keeps update last", async () => {
    const { wrapper, router } = await setup("/system?tab=update");
    try {
      const tabs = wrapper.findAll('[role="tab"]');
      expect(tabs.at(-1)!.text()).toBe("System Update");
      expect(tabs.at(-2)!.text()).toBe("Maintenance");
      expect(tabs.at(-1)!.attributes("data-state")).toBe("active");
      expect(wrapper.find("about-update-stub").exists()).toBe(true);
      expect(router.currentRoute.value.query.tab).toBe("update");
    } finally {
      wrapper.unmount();
    }
  });

  it("keeps the default tab and follows browser history", async () => {
    const { wrapper, router } = await setup("/system");
    try {
      expect(wrapper.findAll('[role="tab"]')[0]!.attributes("data-state")).toBe("active");
      expect(wrapper.find("about-update-stub").exists()).toBe(false);
      await router.push("/system?tab=update");
      await flushPromises();
      expect(wrapper.find("about-update-stub").exists()).toBe(true);
      router.back();
      await flushPromises();
      expect(wrapper.findAll('[role="tab"]')[0]!.attributes("data-state")).toBe("active");
      router.forward();
      await flushPromises();
      expect(wrapper.findAll('[role="tab"]').at(-1)!.attributes("data-state")).toBe("active");
      await wrapper.findAll('[role="tab"]')[0]!.trigger("mousedown", { button: 0, ctrlKey: false });
      await flushPromises();
      expect(router.currentRoute.value.query.tab).toBeUndefined();
      await wrapper.findAll('[role="tab"]').at(-1)!.trigger("mousedown", { button: 0, ctrlKey: false });
      await flushPromises();
      expect(router.currentRoute.value.query.tab).toBe("update");
    } finally {
      wrapper.unmount();
    }
  });
});
