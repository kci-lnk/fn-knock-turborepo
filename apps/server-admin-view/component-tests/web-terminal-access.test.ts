import { flushPromises, mount } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { createI18n } from "vue-i18n";
import { nextTick } from "vue";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { enAdmin } from "../../../packages/i18n/src/messages/admin/en";

const api = vi.hoisted(() => ({ settings: vi.fn(), update: vi.fn() }));
const push = vi.hoisted(() => vi.fn());
vi.mock("@/lib/api/terminal-access", async (original) => ({
  ...(await original<typeof import("@/lib/api/terminal-access")>()),
  TerminalAccessAPI: api,
}));
vi.mock("vue-router", () => ({ useRouter: () => ({ push }) }));
vi.mock("@admin-shared/utils/toast", () => ({
  toast: { success: vi.fn(), error: vi.fn() },
}));
vi.mock("@/views/web-terminal/WebTerminalAuthorized.vue", () => ({
  default: { template: '<div data-test="workspace" />' },
}));

import WebTerminal from "@/views/WebTerminal.vue";
import WebTerminalSettings from "@/views/system-settings/WebTerminalSettings.vue";
import { useTerminalAccessStore } from "@/store/terminal-access";
import { Switch } from "@/components/ui/switch";

const enabled = { enabled: true, revision: "one" };
function options() {
  return {
    global: {
      plugins: [
        createPinia(),
        createI18n({
          legacy: false,
          locale: "en",
          messages: { en: { admin: enAdmin } },
        }),
      ],
      stubs: {
        FloatingActionDock: { template: '<div><slot name="inline" /></div>' },
        RefreshButton: {
          template:
            '<button type="button" @click="$emit(\'click\')">Refresh</button>',
        },
      },
    },
  };
}
beforeEach(() => {
  vi.resetAllMocks();
  vi.useFakeTimers();
  setActivePinia(createPinia());
  api.settings.mockResolvedValue(enabled);
});
afterEach(() => vi.useRealTimers());

async function toggle(wrapper: ReturnType<typeof mount>, value: boolean) {
  wrapper.findComponent(Switch).vm.$emit("update:modelValue", value);
  await nextTick();
}

describe("Web Terminal feature switch", () => {
  it("opens the workspace without a password form, and leaves when disabled", async () => {
    const wrapper = mount(WebTerminal, options());
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    await flushPromises();
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
    expect(wrapper.find("input").exists()).toBe(false);
    api.settings.mockResolvedValue({ enabled: false, revision: "two" });
    await vi.advanceTimersByTimeAsync(5000);
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    expect(push).toHaveBeenCalledWith({
      path: "/system",
      query: { tab: "features" },
    });
    wrapper.unmount();
  });
  it("fails closed on a failed check and recovers on retry", async () => {
    api.settings.mockRejectedValueOnce(new Error("offline"));
    const wrapper = mount(WebTerminal, options());
    await flushPromises();
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    expect(wrapper.get('[role="alert"]').text()).toContain("Request failed");
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Retry")!
      .trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(true);
    wrapper.unmount();
  });
  it("shows only the switch and publishes only a successful save", async () => {
    const wrapper = mount(WebTerminalSettings, options());
    await flushPromises();
    expect(wrapper.find("input").exists()).toBe(false);
    expect(wrapper.findAllComponents(Switch)).toHaveLength(1);
    await toggle(wrapper, false);
    expect(useTerminalAccessStore().status?.enabled).toBe(true);
    api.update.mockRejectedValueOnce(new Error("offline"));
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(wrapper.get('[role="alert"]').text()).toContain("Request failed");
    expect(useTerminalAccessStore().status?.enabled).toBe(true);
    api.update.mockResolvedValue({ enabled: false, revision: "two" });
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.update).toHaveBeenLastCalledWith({
      enabled: false,
      revision: "one",
    });
    expect(useTerminalAccessStore().status?.enabled).toBe(false);
    wrapper.unmount();
  });
  it("saves re-enabling without starting a terminal session", async () => {
    api.settings.mockResolvedValue({ enabled: false, revision: "one" });
    const wrapper = mount(WebTerminalSettings, options());
    await flushPromises();
    await toggle(wrapper, true);
    api.update.mockResolvedValue({ enabled: true, revision: "two" });
    await wrapper.get("form").trigger("submit");
    await flushPromises();
    expect(api.update).toHaveBeenCalledWith({ enabled: true, revision: "one" });
    expect(wrapper.find('[data-test="workspace"]').exists()).toBe(false);
    expect(push).not.toHaveBeenCalled();
    wrapper.unmount();
  });
  it("ignores an old refresh after a settings save", async () => {
    let resolve!: (value: typeof enabled) => void;
    api.settings.mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const store = useTerminalAccessStore();
    const pending = store.refresh();
    store.applySettings({ enabled: false, revision: "two" });
    resolve(enabled);
    await pending;
    expect(store.status?.enabled).toBe(false);
  });
  it("a failed newer check cannot be replaced by an old successful response", async () => {
    const store = useTerminalAccessStore();
    store.applySettings(enabled);
    let resolve!: (value: typeof enabled) => void;
    api.settings.mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const old = store.refresh();
    api.settings.mockRejectedValueOnce(new Error("offline"));
    await expect(store.refresh()).rejects.toThrow("offline");
    resolve(enabled);
    await old;
    expect(store.isCurrent).toBe(false);
  });
  it("keeps a known disabled menu state when a later refresh fails", async () => {
    const store = useTerminalAccessStore();
    store.applySettings({ enabled: false, revision: "disabled" });
    api.settings.mockRejectedValueOnce(new Error("offline"));
    await expect(store.refresh()).rejects.toThrow("offline");
    expect(store.status?.enabled).toBe(false);
    expect(store.isCurrent).toBe(false);
  });
  it("an old failed check cannot replace newer saved settings", async () => {
    const store = useTerminalAccessStore();
    let reject!: (error: Error) => void;
    api.settings.mockImplementationOnce(
      () =>
        new Promise((_, fail) => {
          reject = fail;
        }),
    );
    const old = store.refresh().catch(() => undefined);
    await store.refresh();
    reject(new Error("old failure"));
    await old;
    expect(store.status?.enabled).toBe(true);
  });
  it("coalesces settings refreshes and ignores responses after leaving", async () => {
    let resolve!: (value: typeof enabled) => void;
    api.settings.mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    const wrapper = mount(WebTerminalSettings, options());
    await wrapper
      .findAll("button")
      .find((b) => b.text() === "Refresh")!
      .trigger("click");
    expect(api.settings).toHaveBeenCalledTimes(1);
    wrapper.unmount();
    const store = useTerminalAccessStore();
    store.applySettings({ enabled: false, revision: "two" });
    resolve(enabled);
    await flushPromises();
    expect(store.status?.enabled).toBe(false);
  });
  it("ignores a late save after leaving the settings page", async () => {
    const wrapper = mount(WebTerminalSettings, options());
    await flushPromises();
    let resolve!: (value: typeof enabled) => void;
    api.update.mockImplementationOnce(
      () =>
        new Promise((done) => {
          resolve = done;
        }),
    );
    await toggle(wrapper, false);
    await wrapper.get("form").trigger("submit");
    const store = useTerminalAccessStore();
    wrapper.unmount();
    store.applySettings({ enabled: true, revision: "latest" });
    resolve({ enabled: false, revision: "older-save" });
    await flushPromises();
    expect(store.status?.revision).toBe("latest");
  });
});
