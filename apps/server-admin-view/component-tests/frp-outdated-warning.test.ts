import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import { beforeEach, describe, expect, it, vi } from "vitest";

const runtime = vi.hoisted(() => ({
  installationStatus: "outdated" as "missing" | "outdated" | "current",
  outdatedRunningCount: 1,
  running: true,
  gotoResources: vi.fn(),
}));

vi.mock("../src/views/tunnel/frp/useFrpTunnelController", () => ({
  useFrpTunnelController: () => {
    const primaryInstance = runtime.running
      ? {
          attached: true,
          desiredRunning: true,
          running: true,
          supervisor: {},
        }
      : null;
    return {
      canStart: false,
      canStop: runtime.running,
      configLoaded: true,
      defaults: { local_port: "7999" },
      deleteInstance: vi.fn(),
      deletingInstanceId: null,
      extraInstances: [],
      formatSummary: vi.fn(() => "summary"),
      frpInstallationStatus: runtime.installationStatus,
      frpTargetVersion: "0.71.0",
      getInstanceDisplayName: vi.fn(),
      gotoFrpResources: runtime.gotoResources,
      gotoInstanceCreate: vi.fn(),
      gotoInstanceDetail: vi.fn(),
      isClearingLogs: false,
      isSaving: false,
      isStarting: false,
      isStopping: false,
      onClearLogsClick: vi.fn(),
      overview: {
        outdatedRunningCount: runtime.outdatedRunningCount,
        runningCount: runtime.running ? 1 : 0,
        total: 1,
      },
      pid: runtime.running ? 42 : null,
      primaryConfig: "",
      primaryInstance,
      primaryLogs: [],
      primarySummary: {
        localPort: "7999",
        remotePort: "0",
        serverAddr: "",
        serverPort: "7000",
      },
      saveConfig: vi.fn(),
      setPrimaryEditorRef: vi.fn(),
      showInitDialog:
        runtime.installationStatus === "missing" && !runtime.running,
      startFrpc: vi.fn(),
      startInstance: vi.fn(),
      startingInstanceId: null,
      stopFrpc: vi.fn(),
      stopInstance: vi.fn(),
      stoppingInstanceId: null,
      t: (key: string, params?: Record<string, string>) =>
        params?.version ? `${key}:${params.version}` : key,
    };
  },
}));

vi.mock("vue-i18n", () => ({
  useI18n: () => ({
    t: (key: string, params?: Record<string, string | number>) =>
      params?.version
        ? `${key}:${params.version}`
        : params?.count
          ? `${key}:${params.count}`
          : key,
  }),
}));

import FrpTunnel from "../src/views/tunnel/FrpTunnel.vue";

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
  mount(FrpTunnel, {
    global: {
      stubs: {
        Alert: passthrough,
        AlertDescription: passthrough,
        AlertTitle: passthrough,
        Button: button,
        Card: true,
        CardContent: true,
        CardHeader: true,
        CardTitle: true,
        ConfigCollapsibleCard: true,
        ConfirmDangerPopover: true,
        Dialog: dialog,
        DialogContent: passthrough,
        DialogFooter: passthrough,
        DialogHeader: passthrough,
        DialogTitle: passthrough,
        DocsLinkButton: true,
        FrpcInstanceEditor: true,
        HumanFriendlyTime: true,
        Info: true,
        LogViewer: true,
        Pencil: true,
        Play: true,
        Plus: true,
        ScrollText: true,
        Square: true,
        Trash2: true,
        TriangleAlert: true,
        TunnelSupervisorStatus: true,
      },
    },
  });

describe("FRP tunnel outdated resource warning", () => {
  beforeEach(() => {
    runtime.installationStatus = "outdated";
    runtime.outdatedRunningCount = 1;
    runtime.running = true;
    vi.clearAllMocks();
  });

  it("keeps a running old process available while directing the user to update", async () => {
    const wrapper = mountPage();
    const warning = wrapper.get('[data-testid="frp-outdated-warning"]');

    expect(warning.text()).toContain("admin.frpTunnel.outdatedRunningTitle");
    expect(warning.text()).toContain("0.71.0");
    const stop = wrapper
      .findAll("button")
      .find((candidate) => candidate.text() === "admin.frpTunnel.stop");
    expect(stop).toBeDefined();
    expect(stop?.attributes("disabled")).toBeUndefined();
    await warning.get("button").trigger("click");
    expect(runtime.gotoResources).toHaveBeenCalledTimes(1);
  });

  it("explains that a stopped old process cannot be restarted", () => {
    runtime.running = false;
    runtime.outdatedRunningCount = 0;
    const wrapper = mountPage();
    const warning = wrapper.get('[data-testid="frp-outdated-warning"]');

    expect(warning.text()).toContain("admin.frpTunnel.outdatedStoppedTitle");
    const start = wrapper
      .findAll("button")
      .find((candidate) => candidate.text() === "admin.frpTunnel.start");
    expect(start?.attributes("disabled")).toBeDefined();
  });

  it("keeps reminding after installation until old processes restart", () => {
    runtime.installationStatus = "current";
    const wrapper = mountPage();
    const warning = wrapper.get('[data-testid="frp-outdated-warning"]');

    expect(warning.text()).toContain("admin.frpTunnel.restartRequiredTitle");
    expect(warning.text()).toContain(
      "admin.frpTunnel.restartRequiredDescription",
    );
    expect(warning.find("button").exists()).toBe(false);
  });

  it("directs a running unmanaged old process to install the current version", () => {
    runtime.installationStatus = "missing";
    const wrapper = mountPage();
    const warning = wrapper.get('[data-testid="frp-outdated-warning"]');

    expect(warning.text()).toContain("admin.frpTunnel.outdatedRunningTitle");
    expect(warning.find("button").exists()).toBe(true);
    expect(wrapper.text()).not.toContain("admin.frpTunnel.notInitializedTitle");
  });

  it("keeps the initialization dialog for a missing installation", () => {
    runtime.installationStatus = "missing";
    runtime.outdatedRunningCount = 0;
    runtime.running = false;
    const wrapper = mountPage();

    expect(wrapper.find('[data-testid="frp-outdated-warning"]').exists()).toBe(
      false,
    );
    expect(wrapper.text()).toContain("admin.frpTunnel.notInitializedTitle");
  });
});
