import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { ConfigAPI } from "../src/lib/api/config";
import GatewayPortalSettings from "../src/views/system-settings/GatewayPortalSettings.vue";
import { normalizeGatewayPortalConfig } from "../src/lib/gatewayPortal";
import type { GatewaySettings } from "../src/types";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const mock = vi.hoisted(() => ({
  loadConfig: vi.fn(),
  error: vi.fn(),
  success: vi.fn(),
}));
vi.mock("../src/store/config", () => ({
  useConfigStore: () => ({ loadConfig: mock.loadConfig }),
}));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: mock.error, success: mock.success },
}));

const wrappers: ReturnType<typeof mount>[] = [];
let stored: GatewaySettings;
beforeEach(() => {
  vi.clearAllMocks();
  stored = { portal: normalizeGatewayPortalConfig() } as GatewaySettings;
  vi.spyOn(ConfigAPI, "getGatewaySettings").mockImplementation(
    async () => stored,
  );
  vi.spyOn(ConfigAPI, "updateGatewaySettings").mockImplementation(
    async (patch) => {
      stored = { ...stored, portal: { ...stored.portal, ...patch.portal } };
      return stored;
    },
  );
  mock.loadConfig.mockResolvedValue(undefined);
});
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
});

async function setup() {
  const wrapper = mount(GatewayPortalSettings, {
    global: {
      plugins: [
        createI18n({
          legacy: false,
          locale: "en",
          messages: { en: { admin: enAdmin } },
        }),
      ],
    },
  });
  wrappers.push(wrapper);
  await flushPromises();
  return wrapper;
}
const navigation = (wrapper: ReturnType<typeof mount>) =>
  wrapper.find('[role="group"][aria-label="Navigation mode"]');
const smart = (wrapper: ReturnType<typeof mount>) =>
  wrapper.get('[role="switch"][id$="-smart-lan"]');

describe("portal navigation settings", () => {
  it("shows only smart detection and defaults legacy configuration to off", async () => {
    stored.portal = {
      enabled: true,
      version: "v1",
    } as GatewaySettings["portal"];
    const wrapper = await setup();
    expect(navigation(wrapper).exists()).toBe(false);
    expect(wrapper.text()).not.toContain(
      enAdmin.gatewayPortalSettings.navigationInternet,
    );
    expect(wrapper.text()).not.toContain(
      enAdmin.gatewayPortalSettings.navigationLan,
    );
    expect(smart(wrapper).attributes("aria-checked")).toBe("false");
    expect(ConfigAPI.updateGatewaySettings).not.toHaveBeenCalled();
  });

  it("saves smart detection immediately and preserves the hidden navigation configuration", async () => {
    stored.portal.navigation_mode = "lan";
    const wrapper = await setup();
    await smart(wrapper).trigger("click");
    await flushPromises();
    expect(ConfigAPI.updateGatewaySettings).toHaveBeenLastCalledWith({
      portal: { smart_lan_detection: true },
    });
    const refreshed = await setup();
    expect(smart(refreshed).attributes("aria-checked")).toBe("true");
    expect(navigation(refreshed).exists()).toBe(false);
    await smart(refreshed).trigger("click");
    await flushPromises();
    expect(smart(refreshed).attributes("aria-checked")).toBe("false");
    expect(ConfigAPI.updateGatewaySettings).toHaveBeenLastCalledWith({
      portal: { smart_lan_detection: false },
    });
    expect(stored.portal.navigation_mode).toBe("lan");
  });

  it("rolls back a failed smart save", async () => {
    vi.mocked(ConfigAPI.updateGatewaySettings).mockRejectedValue(
      new Error("Gateway sync failed"),
    );
    const wrapper = await setup();
    await smart(wrapper).trigger("click");
    await flushPromises();
    expect(smart(wrapper).attributes("aria-checked")).toBe("false");
    expect(navigation(wrapper).exists()).toBe(false);
    expect(mock.error).toHaveBeenCalledTimes(1);
    expect(mock.success).not.toHaveBeenCalled();
  });
});
