import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ getConfig: vi.fn() }));
vi.mock("@/lib/api/tunnel", () => ({ CloudflaredAPI: api }));
vi.mock("vue-router", () => ({ useRouter: () => ({ push: vi.fn() }) }));
vi.mock("@/store/config", () => ({
  useConfigStore: () => ({ config: { run_type: 1 }, loadConfig: vi.fn() }),
}));
vi.mock("@/composables/useTargetPolling", () => ({
  useTargetPolling: () => ({ start: vi.fn(), stop: vi.fn() }),
}));
import { useCloudflaredRuntime } from "../src/views/tunnel/cloudflare/useCloudflaredRuntime";

describe("Cloudflare origin address", () => {
  beforeEach(() => vi.clearAllMocks());

  const origins = [17999, 18999, 27999].map(
    (port) => `http://127.0.0.1:${port}`,
  );
  it.each(origins)(
    "uses the backend's dedicated origin %s",
    async (originServiceUrl) => {
      api.getConfig.mockResolvedValue({ originServiceUrl, protocol: "auto" });
      let runtime!: ReturnType<typeof useCloudflaredRuntime>;
      const wrapper = mount(
        defineComponent({
          setup() {
            runtime = useCloudflaredRuntime({
              t: (key) => key,
              onConfigLoaded: vi.fn(),
            });
            return () => h("code", runtime.cloudflaredOriginServiceUrl.value);
          },
        }),
      );
      expect(wrapper.text()).toBe("");
      await runtime.loadConfig();
      expect(wrapper.text()).toBe(originServiceUrl);
      api.getConfig.mockRejectedValueOnce(
        new Error("configuration unavailable"),
      );
      await runtime.loadConfig();
      expect(wrapper.text()).toBe("");
      api.getConfig.mockResolvedValue({ originServiceUrl, protocol: "auto" });
      await runtime.loadConfig();
      expect(wrapper.text()).toBe(originServiceUrl);
      // An older backend must never cause a fallback to the ordinary gateway.
      api.getConfig.mockResolvedValue({ protocol: "auto" });
      await runtime.loadConfig();
      expect(wrapper.text()).toBe("");
      wrapper.unmount();
    },
  );
});
