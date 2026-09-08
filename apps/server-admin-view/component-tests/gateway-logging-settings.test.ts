import { flushPromises, mount } from "@vue/test-utils";
import { createI18n } from "vue-i18n";
import { createMemoryHistory, createRouter } from "vue-router";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GatewayLogsAPI } from "../src/lib/api/gateway";
import { ConfigAPI } from "../src/lib/api/config";
import GatewayLoggingSettings from "../src/views/system-settings/GatewayLoggingSettings.vue";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const mock = vi.hoisted(() => ({
  loadConfig: vi.fn(),
  error: vi.fn(),
  success: vi.fn(),
}));
vi.mock("../src/store/config", () => ({
  useConfigStore: () => ({
    loadConfig: mock.loadConfig,
    isDockerDeployment: false,
  }),
}));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { error: mock.error, success: mock.success },
}));
const initial = {
  enabled: true,
  record_localhost: false,
  max_days: 7,
  logs_dir: "/mnt/old",
  custom_logs_dir: "/mnt/old",
  default_logs_dir: "/runtime/logs",
  dropped_entries: 0,
  queue_size: 0,
  queue_depth: 0,
};
const wrappers: ReturnType<typeof mount>[] = [];
beforeEach(() => {
  vi.clearAllMocks();
  vi.spyOn(GatewayLogsAPI, "getConfig").mockResolvedValue({ ...initial });
  vi.spyOn(GatewayLogsAPI, "updateConfig").mockImplementation(
    async (value) => ({
      ...initial,
      ...value,
      logs_dir: value.custom_logs_dir || initial.default_logs_dir,
      custom_logs_dir: value.custom_logs_dir || "",
    }),
  );
  mock.loadConfig.mockResolvedValue(undefined);
});
afterEach(() => {
  for (const wrapper of wrappers.splice(0)) wrapper.unmount();
  vi.restoreAllMocks();
  document.body.innerHTML = "";
});
const setup = async (hash = "") => {
  const router = createRouter({
    history: createMemoryHistory(),
    routes: [{ path: "/system", component: GatewayLoggingSettings }],
  });
  await router.push(`/system?tab=gateway-logging${hash}`);
  const wrapper = mount(GatewayLoggingSettings, {
    attachTo: document.body,
    global: {
      plugins: [
        router,
        createI18n({
          legacy: false,
          locale: "en",
          messages: { en: { admin: enAdmin, common: { cancel: "Cancel" } } },
        }),
      ],
      stubs: {
        FloatingActionDock: { template: '<div><slot name="inline" /></div>' },
      },
    },
  });
  wrappers.push(wrapper);
  await flushPromises();
  return wrapper;
};
const button = (wrapper: ReturnType<typeof mount>, text: string) => {
  const result = wrapper.findAll("button").find((item) => item.text() === text);
  if (!result) throw new Error(`Missing button: ${text}`);
  return result;
};

describe("request log storage settings", () => {
  it("stages directory changes, reverts edits, and saves a default reset explicitly", async () => {
    const wrapper = await setup();
    const input = wrapper.get("#gateway-log-directory");
    expect(wrapper.text()).toContain("Active location: /mnt/old");
    expect(wrapper.text()).toContain("Default location: /runtime/logs");
    await input.setValue("/mnt/日志 folder");
    expect(GatewayLogsAPI.updateConfig).not.toHaveBeenCalled();
    await button(wrapper, enAdmin.gatewayLogging.reset).trigger("click");
    expect((input.element as HTMLInputElement).value).toBe("/mnt/old");
    await button(wrapper, "Restore default location").trigger("click");
    expect((input.element as HTMLInputElement).value).toBe("");
    expect(GatewayLogsAPI.updateConfig).not.toHaveBeenCalled();
    await button(wrapper, enAdmin.gatewayLogging.saveSettings).trigger("click");
    await flushPromises();
    expect(GatewayLogsAPI.updateConfig).toHaveBeenCalledWith({
      enabled: true,
      record_localhost: false,
      max_days: 7,
      custom_logs_dir: "",
    });
    expect(wrapper.text()).toContain("Active location: /runtime/logs");
  });
  it("keeps the draft and active path when saving fails", async () => {
    vi.mocked(GatewayLogsAPI.updateConfig).mockRejectedValue(
      new Error("Permission denied"),
    );
    const wrapper = await setup();
    await wrapper.get("#gateway-log-directory").setValue("/read-only/logs");
    await button(wrapper, enAdmin.gatewayLogging.saveSettings).trigger("click");
    await flushPromises();
    expect(mock.error).toHaveBeenCalled();
    expect(wrapper.text()).toContain("Active location: /mnt/old");
    expect(
      (wrapper.get("#gateway-log-directory").element as HTMLInputElement).value,
    ).toBe("/read-only/logs");
    expect(
      button(wrapper, enAdmin.gatewayLogging.saveSettings).attributes(
        "disabled",
      ),
    ).toBeUndefined();
  });
  it("refreshes the actual directory after an uncertain save while preserving the draft", async () => {
    const wrapper = await setup();
    vi.mocked(GatewayLogsAPI.updateConfig).mockRejectedValue(
      new Error("Gateway timeout"),
    );
    vi.mocked(GatewayLogsAPI.getConfig).mockResolvedValue({
      ...initial,
      logs_dir: "/mnt/active-after-timeout",
    });
    await wrapper.get("#gateway-log-directory").setValue("/mnt/draft");
    await button(wrapper, enAdmin.gatewayLogging.saveSettings).trigger("click");
    await flushPromises();
    expect(wrapper.text()).toContain(
      "Active location: /mnt/active-after-timeout",
    );
    expect(
      (wrapper.get("#gateway-log-directory").element as HTMLInputElement).value,
    ).toBe("/mnt/draft");
  });
  it("selects a directory through the existing browser without saving immediately", async () => {
    vi.spyOn(ConfigAPI, "browseHostMappingStaticPath").mockResolvedValue({
      breadcrumbs: [],
      current_path: "/mnt/new",
      current_selectable: true,
      entries: [],
      error_code: null,
      next_cursor: null,
      parent_path: "/mnt",
      platform: "posix",
      previous_cursor: null,
      selected_path: null,
      target_type: "directory",
    });
    vi.spyOn(ConfigAPI, "probeHostMappingStaticPath").mockResolvedValue({
      actual_type: "directory",
      error_code: null,
      exists: true,
      normalized_path: "/mnt/new",
      readable: true,
      target_type: "directory",
    });
    const wrapper = await setup();
    await button(wrapper, "Browse folders").trigger("click");
    await flushPromises();
    expect(ConfigAPI.browseHostMappingStaticPath).toHaveBeenCalledWith(
      "directory",
      "/mnt/old",
      null,
    );
    const choose = [...document.querySelectorAll("button")].find(
      (item) => item.textContent?.trim() === "Use this folder",
    );
    expect(choose).toBeTruthy();
    choose!.click();
    await flushPromises();
    expect(ConfigAPI.probeHostMappingStaticPath).toHaveBeenCalledWith(
      "directory",
      "/mnt/new",
    );
    expect(
      (wrapper.get("#gateway-log-directory").element as HTMLInputElement).value,
    ).toBe("/mnt/new");
    expect(GatewayLogsAPI.updateConfig).not.toHaveBeenCalled();
  });
  it("focuses the storage input after following the observation link", async () => {
    const wrapper = await setup("#gateway-log-directory");
    expect(document.activeElement).toBe(
      wrapper.get("#gateway-log-directory").element,
    );
  });
});
