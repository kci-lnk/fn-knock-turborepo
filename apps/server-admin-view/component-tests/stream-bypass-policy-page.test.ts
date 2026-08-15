import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { defineComponent } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { ConfigAPI } from "../src/lib/api/config";
import { useConfigStore } from "../src/store/config";
import type { AppConfig, StreamMapping } from "../src/types";
import { useStreamBypassPolicyPage } from "../src/views/stream-mappings/useStreamBypassPolicyPage";

const routerMocks = vi.hoisted(() => ({
  push: vi.fn(),
  route: { params: { port: "6006", protocol: "tcp" } },
}));

vi.mock("vue-router", async (importOriginal) => ({
  ...(await importOriginal<typeof import("vue-router")>()),
  onBeforeRouteLeave: vi.fn(),
  onBeforeRouteUpdate: vi.fn(),
  useRoute: () => routerMocks.route,
  useRouter: () => ({ push: routerMocks.push }),
}));

const mapping: StreamMapping = {
  comment: "VNC",
  listen_port: 6006,
  protocol: "tcp",
  target: "192.0.2.10:5900",
  use_auth: true,
};

const Harness = defineComponent({
  setup() {
    return { model: useStreamBypassPolicyPage() };
  },
  template: '<div data-testid="loading">{{ model.loading }}</div>',
});

describe("stream bypass policy page loading", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.restoreAllMocks();
  });

  it("reuses the config already loaded by Layout instead of reloading it on mount", async () => {
    const configStore = useConfigStore();
    configStore.config = {
      host_mapping_grouped_view: false,
      host_mapping_groups: [],
      host_mappings: [],
      stream_mappings: [mapping],
    } as AppConfig;
    const loadConfig = vi.spyOn(configStore, "loadConfig");
    const getPolicy = vi
      .spyOn(ConfigAPI, "getStreamBypassPolicy")
      .mockResolvedValue({
        broad_rule_confirmed: false,
        enabled: false,
        groups: [],
        policy_version: "version-1",
      });

    const wrapper = mount(Harness, {
      global: {
        plugins: [
          createI18n({
            legacy: false,
            locale: "en",
            messages: { en: {} },
            missingWarn: false,
            fallbackWarn: false,
          }),
        ],
      },
    });
    await flushPromises();

    expect(loadConfig).not.toHaveBeenCalled();
    expect(getPolicy).toHaveBeenCalledOnce();
    expect(wrapper.get('[data-testid="loading"]').text()).toBe("false");
    wrapper.unmount();
  });
});
