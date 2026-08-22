import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent, h } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AppConfig } from "../src/types";

const api = vi.hoisted(() => ({
  getAuthCredentialSettings: vi.fn(),
  getProtocolMappingFeatureConfig: vi.fn(),
  getWOLFeature: vi.fn(),
  updateDashboardDisplayConfig: vi.fn(),
  updateProtocolMappingFeatureConfig: vi.fn(),
}));

vi.mock("@/lib/api/config", () => ({
  ConfigAPI: {
    getAuthCredentialSettings: api.getAuthCredentialSettings,
    getWOLFeature: api.getWOLFeature,
    updateDashboardDisplayConfig: api.updateDashboardDisplayConfig,
  },
}));

vi.mock("@/lib/api/system", () => ({
  SystemAPI: {
    getProtocolMappingFeatureConfig: api.getProtocolMappingFeatureConfig,
    updateProtocolMappingFeatureConfig: api.updateProtocolMappingFeatureConfig,
  },
}));

vi.mock("@/lib/api/security", () => ({ SSHSecurityAPI: {} }));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn() },
}));
vi.mock("vue-router", () => ({ useRouter: () => ({ push: vi.fn() }) }));

import { useConfigStore } from "../src/store/config";
import FeaturesSettings from "../src/views/system-settings/FeaturesSettings.vue";
import { useFeaturesSettings } from "../src/views/system-settings/useFeaturesSettings";

const fpkConfig = (): AppConfig =>
  ({
    capabilities: {
      auto_https_available: false,
      host_firewall_available: false,
    },
    dashboard_display: {
      date_time_display_mode: "human_friendly",
      show_console_app_list: false,
      show_entry_status_module: true,
    },
    run_type: 0,
    runtime_profile: { deployment_target: "fpk" },
  }) as AppConfig;

const mountSettings = (config: AppConfig) => {
  const pinia = createPinia();
  setActivePinia(pinia);
  const store = useConfigStore();
  store.config = config;
  store.isLoading = false;
  let settings!: ReturnType<typeof useFeaturesSettings>;
  const harness = defineComponent({
    setup() {
      settings = useFeaturesSettings();
      return () => h("div");
    },
  });
  const i18n = createI18n({ legacy: false, locale: "en" });
  const wrapper = mount(harness, { global: { plugins: [pinia, i18n] } });
  return { settings, store, wrapper };
};

const FeatureSwitchRowStub = defineComponent({
  props: { title: { type: String, required: true } },
  setup(props) {
    return () => h("div", { "data-feature-title": props.title });
  },
});

const mountFeatureView = (config: AppConfig) => {
  const pinia = createPinia();
  setActivePinia(pinia);
  const store = useConfigStore();
  store.config = config;
  store.isLoading = false;
  const i18n = createI18n({ legacy: false, locale: "en" });
  return mount(FeaturesSettings, {
    global: {
      plugins: [pinia, i18n],
      stubs: {
        DateTimeDisplaySettingRow: true,
        FeatureSwitchRow: FeatureSwitchRowStub,
        SmartConnectSettingRow: true,
      },
    },
  });
};

describe("FPK console application list setting", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    api.getAuthCredentialSettings.mockResolvedValue({
      passkey_bind_prompt_enabled: true,
    });
    api.getProtocolMappingFeatureConfig.mockResolvedValue({ enabled: false });
    api.getWOLFeature.mockResolvedValue({ enabled: false });
  });

  it("optimistically updates and rolls back when saving fails", async () => {
    const { settings, wrapper } = mountSettings(fpkConfig());
    await flushPromises();

    let rejectSave: ((error: Error) => void) | undefined;
    api.updateDashboardDisplayConfig.mockReturnValueOnce(
      new Promise((_resolve, reject) => {
        rejectSave = reject;
      }),
    );

    const pending = settings.saveShowConsoleAppList(true);
    expect(settings.showConsoleAppList.value).toBe(true);
    expect(api.updateDashboardDisplayConfig).toHaveBeenCalledWith({
      show_console_app_list: true,
    });

    rejectSave?.(new Error("save failed"));
    await pending;
    expect(settings.showConsoleAppList.value).toBe(false);
    wrapper.unmount();
  });

  it("refreshes repair navigation state after protocol mapping startup is rejected", async () => {
    const { settings, store, wrapper } = mountSettings({
      ...fpkConfig(),
      run_type: 3,
    });
    await flushPromises();
    const loadConfig = vi
      .spyOn(store, "loadConfig")
      .mockResolvedValue(store.config);
    api.updateProtocolMappingFeatureConfig.mockRejectedValueOnce(
      new Error("listen tcp :9000: bind: address already in use"),
    );

    await settings.saveProtocolMappingEnabled(true);

    expect(settings.protocolMappingEnabled.value).toBe(false);
    expect(loadConfig).toHaveBeenCalledWith({ force: true });
    wrapper.unmount();
  });

  it("does not expose or persist the setting outside FPK", async () => {
    const config = {
      ...fpkConfig(),
      runtime_profile: { deployment_target: "docker" },
    } as AppConfig;
    const { settings, wrapper } = mountSettings(config);
    await flushPromises();

    expect(settings.showConsoleAppListEntry.value).toBe(false);
    await settings.saveShowConsoleAppList(true);
    expect(settings.showConsoleAppList.value).toBe(false);
    expect(api.updateDashboardDisplayConfig).not.toHaveBeenCalled();
    wrapper.unmount();
  });

  it("does not render the configuration row outside FPK", async () => {
    const selector =
      '[data-feature-title="admin.featuresSettings.showConsoleAppList"]';
    const fpkWrapper = mountFeatureView(fpkConfig());
    await flushPromises();
    expect(fpkWrapper.find(selector).exists()).toBe(true);
    fpkWrapper.unmount();

    const config = {
      ...fpkConfig(),
      runtime_profile: { deployment_target: "docker" },
    } as AppConfig;
    const wrapper = mountFeatureView(config);
    await flushPromises();

    expect(wrapper.find(selector).exists()).toBe(false);
    wrapper.unmount();
  });
});
