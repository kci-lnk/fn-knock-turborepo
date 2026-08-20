import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const runtime = vi.hoisted(() => ({
  installationStatus: "outdated" as "missing" | "outdated" | "current",
  running: true,
  gotoResources: vi.fn(),
}));

vi.mock("../src/views/tunnel/cloudflare/useCloudflareTunnelController", () => ({
  useCloudflareTunnelController: () => ({
    canStart: false,
    canStop: true,
    cloudflaredInstallationStatus: runtime.installationStatus,
    cloudflaredLogAnalysis: null,
    cloudflaredLogAnalysisMessage: "",
    cloudflaredTargetVersion: "2026.7.3",
    configLoaded: true,
    gotoResources: runtime.gotoResources,
    hasSubdomainRoot: true,
    isClearingLogs: false,
    isReverseProxySubdomainMode: true,
    isStarting: false,
    isStopping: false,
    logs: [],
    onClearLogsClick: vi.fn(),
    pid: runtime.running ? 42 : null,
    running: runtime.running,
    showInitDialog: runtime.installationStatus === "missing",
    startCloudflared: vi.fn(),
    stopCloudflared: vi.fn(),
    supervisor: {
      desiredRunning: runtime.running,
      running: runtime.running,
    },
    t: (key: string, params?: Record<string, string>) =>
      params?.version ? `${key}:${params.version}` : key,
  }),
}));

import CloudflareTunnel from "../src/views/tunnel/CloudflareTunnel.vue";

const passthrough = defineComponent({
  setup(_, { attrs, slots }) {
    return () => h("div", attrs, slots.default?.());
  },
});

const button = defineComponent({
  inheritAttrs: false,
  setup(_, { attrs, slots }) {
    return () => h("button", attrs, slots.default?.());
  },
});

const dialog = defineComponent({
  props: { open: Boolean },
  setup(props, { slots }) {
    return () => (props.open ? h("div", slots.default?.()) : null);
  },
});

const mountPage = () =>
  mount(CloudflareTunnel, {
    global: {
      stubs: {
        Alert: passthrough,
        AlertDescription: passthrough,
        AlertTitle: passthrough,
        Button: button,
        CloudflareApiConnectionCard: true,
        CloudflareManagedTunnelCard: true,
        CloudflareManualConfigCard: true,
        CloudflareOptimizationCard: true,
        ConfigCollapsibleCard: true,
        Dialog: dialog,
        DialogContent: passthrough,
        DialogFooter: passthrough,
        DialogHeader: passthrough,
        DialogTitle: passthrough,
        LoaderCircle: true,
        TriangleAlert: true,
      },
    },
  });

describe("Cloudflare tunnel outdated Cloudflared warning", () => {
  beforeEach(() => {
    runtime.installationStatus = "outdated";
    runtime.running = true;
    vi.clearAllMocks();
  });

  it("keeps a running old process available while directing the user to update", async () => {
    const wrapper = mountPage();
    const warning = wrapper.get('[data-testid="cloudflared-outdated-warning"]');

    expect(warning.text()).toContain(
      "admin.cloudflareTunnel.outdatedRunningTitle",
    );
    expect(warning.text()).toContain("2026.7.3");
    const stop = wrapper
      .findAll("button")
      .find((candidate) => candidate.text() === "admin.cloudflareTunnel.stop");
    expect(stop).toBeDefined();
    expect(stop?.attributes("disabled")).toBeUndefined();
    await warning.get("button").trigger("click");
    expect(runtime.gotoResources).toHaveBeenCalledTimes(1);
  });

  it("explains that a stopped old process cannot be restarted", () => {
    runtime.running = false;
    const warning = mountPage().get(
      '[data-testid="cloudflared-outdated-warning"]',
    );

    expect(warning.text()).toContain(
      "admin.cloudflareTunnel.outdatedStoppedTitle",
    );
    const start = warning.element
      .closest(".space-y-6")
      ?.querySelector<HTMLButtonElement>("button");
    expect(start?.textContent).toContain("admin.cloudflareTunnel.start");
    expect(start?.disabled).toBe(true);
  });

  it("keeps the initialization dialog for a missing installation", () => {
    runtime.installationStatus = "missing";
    const wrapper = mountPage();

    expect(
      wrapper.find('[data-testid="cloudflared-outdated-warning"]').exists(),
    ).toBe(false);
    expect(wrapper.text()).toContain(
      "admin.cloudflareTunnel.notInitializedTitle",
    );
  });
});
