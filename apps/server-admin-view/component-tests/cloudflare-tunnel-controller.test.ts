import { flushPromises, mount } from "@vue/test-utils";
import { defineComponent, h, ref } from "vue";
import { createI18n } from "vue-i18n";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CloudflaredConfig } from "../src/lib/api/tunnel";

const lifecycle = vi.hoisted(() => ({
  applyManagedConfig: vi.fn(),
  loadAccessEntryPort: vi.fn(),
  loadConfig: vi.fn(),
  loadEnvironmentConfig: vi.fn(),
  loadManagedState: vi.fn(),
  loadStatus: vi.fn(),
  recoverReconcile: vi.fn(),
  startManagedPolling: vi.fn(),
  startRuntimePolling: vi.fn(),
  stopManaged: vi.fn(),
  stopOptimization: vi.fn(),
  stopRuntimePolling: vi.fn(),
}));

vi.mock("../src/views/tunnel/cloudflare/useCloudflaredRuntime", () => ({
  useCloudflaredRuntime: ({
    onConfigLoaded,
  }: {
    onConfigLoaded: (config: CloudflaredConfig) => void;
  }) => ({
    loadAccessEntryPort: async () => lifecycle.loadAccessEntryPort(),
    loadConfig: async () => {
      lifecycle.loadConfig();
      onConfigLoaded({} as CloudflaredConfig);
    },
    loadEnvironmentConfig: async () => lifecycle.loadEnvironmentConfig(),
    loadStatus: async () => lifecycle.loadStatus(),
    startPolling: lifecycle.startRuntimePolling,
    stopPolling: lifecycle.stopRuntimePolling,
    tunnelTokenConfigured: ref(false),
  }),
}));

vi.mock("../src/views/tunnel/cloudflare/useCloudflareManagedTunnel", () => ({
  useCloudflareManagedTunnel: () => ({
    applyConfig: lifecycle.applyManagedConfig,
    loadManagedState: async () => lifecycle.loadManagedState(),
    managedState: ref(null),
    optimizationEnabled: ref(false),
    prepareOptimizationConflictResolution: vi.fn(),
    previewReconcile: vi.fn(),
    reconcilePlan: ref(null),
    recoverActiveReconcileJob: async () => lifecycle.recoverReconcile(),
    startPolling: lifecycle.startManagedPolling,
    stop: lifecycle.stopManaged,
  }),
}));

vi.mock("../src/views/tunnel/cloudflare/useCloudflareOptimization", () => ({
  useCloudflareOptimization: () => ({ stop: lifecycle.stopOptimization }),
}));

import { useCloudflareTunnelController } from "../src/views/tunnel/cloudflare/useCloudflareTunnelController";

describe("useCloudflareTunnelController", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("coordinates initialization and owns teardown without domain API logic", async () => {
    const component = defineComponent({
      setup() {
        useCloudflareTunnelController();
        return () => h("div");
      },
    });
    const i18n = createI18n({ legacy: false, locale: "en" });
    const wrapper = mount(component, { global: { plugins: [i18n] } });
    await flushPromises();

    for (const initialized of [
      lifecycle.loadAccessEntryPort,
      lifecycle.loadConfig,
      lifecycle.loadEnvironmentConfig,
      lifecycle.loadManagedState,
      lifecycle.loadStatus,
      lifecycle.recoverReconcile,
      lifecycle.startManagedPolling,
      lifecycle.startRuntimePolling,
    ]) {
      expect(initialized).toHaveBeenCalledTimes(1);
    }
    expect(lifecycle.applyManagedConfig).toHaveBeenCalledTimes(1);

    wrapper.unmount();
    expect(lifecycle.stopManaged).toHaveBeenCalledTimes(1);
    expect(lifecycle.stopOptimization).toHaveBeenCalledTimes(1);
    expect(lifecycle.stopRuntimePolling).toHaveBeenCalledTimes(1);
  });

  it("does not start polling when initialization finishes after unmount", async () => {
    let resolveStatus: (() => void) | undefined;
    lifecycle.loadStatus.mockReturnValueOnce(
      new Promise<void>((resolve) => {
        resolveStatus = resolve;
      }),
    );

    const component = defineComponent({
      setup() {
        useCloudflareTunnelController();
        return () => h("div");
      },
    });
    const i18n = createI18n({ legacy: false, locale: "en" });
    const wrapper = mount(component, { global: { plugins: [i18n] } });
    await Promise.resolve();

    wrapper.unmount();
    resolveStatus?.();
    await flushPromises();

    expect(lifecycle.startManagedPolling).not.toHaveBeenCalled();
    expect(lifecycle.startRuntimePolling).not.toHaveBeenCalled();
  });
});
